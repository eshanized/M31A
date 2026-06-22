export default function Home() {
  return (
    <main className="flex-1">
      <section className="flex min-h-[80vh] items-center justify-center">
        <div className="container mx-auto px-4 text-center">
          <h1 className="text-5xl font-bold tracking-tight">
            Welcome to <span className="text-primary">Acme</span>
          </h1>
          <p className="mx-auto mt-6 max-w-2xl text-lg text-text-secondary">
            Build something amazing with our platform.
          </p>
        </div>
      </section>
    </main>
  );
}
