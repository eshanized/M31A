"""Example: Snapshot checkpoints and safe recovery."""

import asyncio
from m31a import M31AClient

async def main():
    async with M31AClient() as client:
        thread = await client.create_thread()
        print(f"Created Session Thread: {thread.id}")

        # Create a checkpoint before making modifications
        checkpoint = await thread.create_checkpoint("Pre-Refactor Snapshot", ["m31a/"])
        print(f"Created Checkpoint: {checkpoint.id} - '{checkpoint.label}'")

        # Run automated turn with approval policy
        turn = await thread.run(
            "Refactor exception hierarchy and run checks.",
            on_approval=lambda req: "allow" if "rm" not in (req.command or "") else "deny",
            on_content=lambda chunk: print(chunk, end="", flush=True),
        )

        await turn.result()

        # If needed, restore session and workspace state back to checkpoint
        print("\nRestoring checkpoint...")
        restored = await thread.restore_checkpoint(checkpoint.id, revert_conversation=True)
        print(f"Restored to turn index {restored.turn_index} successfully.")

if __name__ == "__main__":
    asyncio.run(main())
