import Link from "next/link";

export default function NotFound() {
  return (
    <main className="flex flex-1 items-center justify-center">
      <div className="container mx-auto px-4 text-center">
        <p className="text-6xl font-bold text-primary">404</p>
        <h1 className="mt-4 text-3xl font-bold">Page Not Found</h1>
        <p className="mt-2 text-text-secondary">
          The page you are looking for does not exist or has been moved.
        </p>
        <Link
          href="/"
          className="mt-8 inline-flex items-center rounded-lg bg-primary px-6 py-3 text-sm font-medium text-white transition-colors hover:bg-primary-hover"
        >
          Back to Home
        </Link>
      </div>
    </main>
  );
}
