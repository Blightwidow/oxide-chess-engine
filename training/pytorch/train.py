"""Oxide NNUE training loop with MPS/CUDA/CPU support."""

import argparse
import csv
import os
import sys
import time
from pathlib import Path

import torch
import torch.nn as nn

from config import (
    BATCH_SIZE,
    BATCHES_PER_SUPERBATCH,
    CHECKPOINT_DIR,
    END_SUPERBATCH,
    EVAL_SCALE,
    LEARNING_RATE,
    LR_GAMMA,
    LR_STEP,
    SAVE_RATE,
    get_device,
)
from data import create_dataloader
from model import OxideNNUE


def save_checkpoint(
    model: OxideNNUE,
    optimizer: torch.optim.Optimizer,
    superbatch: int,
    checkpoint_directory: str,
):
    """Save model and optimizer state."""
    checkpoint_path = Path(checkpoint_directory) / f"oxide-{superbatch}"
    checkpoint_path.mkdir(parents=True, exist_ok=True)

    torch.save(model.state_dict(), checkpoint_path / "model.pt")
    torch.save(optimizer.state_dict(), checkpoint_path / "optimizer.pt")
    torch.save({"superbatch": superbatch}, checkpoint_path / "meta.pt")
    print(f"  Checkpoint saved: {checkpoint_path}")


def load_checkpoint(
    model: OxideNNUE,
    optimizer: torch.optim.Optimizer,
    checkpoint_path: str,
    device: torch.device,
) -> int:
    """Load model and optimizer state. Returns the superbatch number to resume from."""
    checkpoint_path = Path(checkpoint_path)
    model.load_state_dict(torch.load(checkpoint_path / "model.pt", map_location=device, weights_only=True))
    optimizer.load_state_dict(torch.load(checkpoint_path / "optimizer.pt", map_location=device, weights_only=True))
    meta = torch.load(checkpoint_path / "meta.pt", map_location=device, weights_only=True)
    return meta["superbatch"]


def compute_learning_rate(superbatch: int) -> float:
    """StepLR schedule: decay by gamma every LR_STEP superbatches."""
    return LEARNING_RATE * (LR_GAMMA ** (superbatch // LR_STEP))


def validate(
    model: OxideNNUE,
    validation_path: str,
    device: torch.device,
    max_batches: int = 64,
) -> float:
    """Compute validation loss."""
    model.eval()
    dataset = create_dataloader(validation_path, batch_size=BATCH_SIZE, shuffle=False)

    total_loss = 0.0
    num_batches = 0

    with torch.no_grad():
        for batch in dataset:
            stm_indices, stm_offsets, ntm_indices, ntm_offsets, targets = batch
            stm_indices = stm_indices.to(device)
            stm_offsets = stm_offsets.to(device)
            ntm_indices = ntm_indices.to(device)
            ntm_offsets = ntm_offsets.to(device)
            targets = targets.to(device)

            output = model(stm_indices, stm_offsets, ntm_indices, ntm_offsets)
            loss = nn.functional.mse_loss(torch.sigmoid(output), targets)
            total_loss += loss.item()
            num_batches += 1

            if num_batches >= max_batches:
                break

    model.train()
    return total_loss / max(num_batches, 1)


def train(
    data_path: str,
    validation_path: str | None = None,
    resume_from: str | None = None,
    checkpoint_directory: str = CHECKPOINT_DIR,
    end_superbatch: int = END_SUPERBATCH,
    save_rate: int = SAVE_RATE,
):
    """Main training loop."""
    device = get_device()
    print(f"Device: {device}")

    # Set MPS fallback for unsupported ops
    if device.type == "mps":
        os.environ.setdefault("PYTORCH_ENABLE_MPS_FALLBACK", "1")

    model = OxideNNUE().to(device)
    total_parameters = sum(parameter.numel() for parameter in model.parameters())
    print(f"Model parameters: {total_parameters:,}")

    optimizer = torch.optim.AdamW(model.parameters(), lr=LEARNING_RATE)

    start_superbatch = 1
    if resume_from:
        start_superbatch = load_checkpoint(model, optimizer, resume_from, device) + 1
        print(f"Resuming from superbatch {start_superbatch}")

    # Open training log
    log_path = Path(checkpoint_directory) / "log.csv"
    log_path.parent.mkdir(parents=True, exist_ok=True)
    log_file = open(log_path, "a", newline="")
    log_writer = csv.writer(log_file)
    if log_path.stat().st_size == 0:
        log_writer.writerow(["superbatch", "batch", "loss"])

    # Validation log
    validation_log_path = Path(checkpoint_directory) / "val_log.txt"

    print(f"Training superbatches {start_superbatch} to {end_superbatch}")
    print(f"  Batches per superbatch: {BATCHES_PER_SUPERBATCH}")
    print(f"  Batch size: {BATCH_SIZE}")
    print(f"  Positions per superbatch: ~{BATCHES_PER_SUPERBATCH * BATCH_SIZE / 1e6:.0f}M")
    print()

    model.train()

    for superbatch in range(start_superbatch, end_superbatch + 1):
        # Update learning rate (StepLR)
        current_learning_rate = compute_learning_rate(superbatch)
        for parameter_group in optimizer.param_groups:
            parameter_group["lr"] = current_learning_rate

        superbatch_loss = 0.0
        batch_count = 0
        superbatch_start_time = time.time()

        # Create fresh dataset iterator for each superbatch (reshuffles)
        dataset = create_dataloader(data_path, batch_size=BATCH_SIZE, shuffle=True)

        for batch in dataset:
            stm_indices, stm_offsets, ntm_indices, ntm_offsets, targets = batch
            stm_indices = stm_indices.to(device)
            stm_offsets = stm_offsets.to(device)
            ntm_indices = ntm_indices.to(device)
            ntm_offsets = ntm_offsets.to(device)
            targets = targets.to(device)

            optimizer.zero_grad()
            output = model(stm_indices, stm_offsets, ntm_indices, ntm_offsets)
            loss = nn.functional.mse_loss(torch.sigmoid(output), targets)
            loss.backward()
            optimizer.step()

            superbatch_loss += loss.item()
            batch_count += 1

            # Log and print progress every 32 batches
            if batch_count % 32 == 0:
                log_writer.writerow([superbatch, batch_count, f"{loss.item():.6f}"])
                log_file.flush()
                print(
                    f"\r  [{superbatch}/{end_superbatch}] batch {batch_count}/{BATCHES_PER_SUPERBATCH} "
                    f"loss={loss.item():.5f}",
                    end="",
                    flush=True,
                )

            if batch_count >= BATCHES_PER_SUPERBATCH:
                break

        elapsed = time.time() - superbatch_start_time
        average_loss = superbatch_loss / max(batch_count, 1)
        positions_per_second = (batch_count * BATCH_SIZE) / elapsed

        print(
            f"Superbatch {superbatch}/{end_superbatch} | "
            f"loss={average_loss:.5f} | "
            f"lr={current_learning_rate:.6f} | "
            f"{positions_per_second:.0f} pos/s | "
            f"{elapsed:.1f}s"
        )

        # Checkpoint
        if superbatch % save_rate == 0 or superbatch == end_superbatch:
            save_checkpoint(model, optimizer, superbatch, checkpoint_directory)

            # Validation
            if validation_path and Path(validation_path).exists():
                validation_loss = validate(model, validation_path, device)
                validation_line = f"superbatch {superbatch} | val_loss = {validation_loss:.5f}"
                print(f"  [Validation] {validation_line}")
                with open(validation_log_path, "a") as validation_file:
                    validation_file.write(validation_line + "\n")

    log_file.close()
    print("Training complete.")


def main():
    parser = argparse.ArgumentParser(description="Oxide NNUE PyTorch Trainer")
    parser.add_argument("data", help="Path to preprocessed .bin data file")
    parser.add_argument("--validation", help="Path to preprocessed validation .bin file")
    parser.add_argument("--resume", help="Path to checkpoint directory to resume from")
    parser.add_argument("--checkpoint-dir", default=CHECKPOINT_DIR, help="Checkpoint output directory")
    parser.add_argument("--superbatches", type=int, default=END_SUPERBATCH, help="Total superbatches to train")
    parser.add_argument("--save-rate", type=int, default=SAVE_RATE, help="Save checkpoint every N superbatches")
    arguments = parser.parse_args()

    train(
        data_path=arguments.data,
        validation_path=arguments.validation,
        resume_from=arguments.resume,
        checkpoint_directory=arguments.checkpoint_dir,
        end_superbatch=arguments.superbatches,
        save_rate=arguments.save_rate,
    )


if __name__ == "__main__":
    main()
