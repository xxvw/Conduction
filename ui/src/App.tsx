// 初期の骨格。UIキット.jsx は設計資産として ui/src/components/ 配下に保管され、
// Phase 2 以降で React/TypeScript に移植しながらここに組み込む。

export function App() {
  return (
    <main
      style={{
        minHeight: "100vh",
        display: "grid",
        placeItems: "center",
        background: "var(--c-ink-1)",
        color: "var(--c-ink-11)",
        fontFamily: "var(--font-sans)",
      }}
    >
      <div style={{ textAlign: "center" }}>
        <h1 style={{ fontFamily: "var(--font-display)", fontSize: "var(--fs-h1)", margin: 0 }}>
          Conduction
        </h1>
        <p style={{ color: "var(--c-ink-9)", fontSize: "var(--fs-small)", marginTop: "var(--s-4)" }}>
          Conduct your mix, don&apos;t perform it.
        </p>
      </div>
    </main>
  );
}
