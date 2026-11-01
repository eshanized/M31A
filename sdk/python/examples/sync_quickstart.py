"""Example: Synchronous quickstart script without async boilerplate."""

from m31a import SyncM31AClient

def main():
    with SyncM31AClient() as client:
        print("Connected to M31A App Server via synchronous wrapper.")
        diag = client.get_diagnostics()
        print(f"Health Status: {diag.status}, Model: {diag.active_model}")

        thread = client.create_thread(cwd=".")
        print(f"Created Session Thread: {thread.id}")

        turn = thread.run(
            "Explain the difference between threading and multiprocessing in Python.",
            on_content=lambda chunk: print(chunk, end="", flush=True),
        )

        output = turn.result()
        print("\n\nDone! Length:", len(output))

if __name__ == "__main__":
    main()
