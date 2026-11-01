"""Example: Async streaming turn with DeepSeek R1 / Nemotron reasoning path inspection."""

import asyncio
import sys
from m31a import M31AClient

async def main():
    async with M31AClient() as client:
        print("Connected to M31A App Server.")
        diagnostics = await client.get_diagnostics()
        print(f"Active Model: {diagnostics.active_model}")

        thread = await client.create_thread()
        print(f"Initialized Thread: {thread.id}")

        turn = await thread.run(
            "Write a Python dataclass representing an asynchronous task with status validation.",
            on_reasoning=lambda chunk: sys.stderr.write(f"\033[35m{chunk}\033[0m"),
            on_content=lambda chunk: sys.stdout.write(chunk),
            on_tool_started=lambda ev: print(f"\n\033[33m[Tool Invoked: {ev.get('tool')}]\033[0m"),
            on_tool_finished=lambda ev: print(f"\033[32m[Tool Finished: {ev.get('call_id')}]\033[0m"),
        )

        final_result = await turn.result()
        print("\n\n--- Final Assistant Output ---\n", final_result)

if __name__ == "__main__":
    asyncio.run(main())
