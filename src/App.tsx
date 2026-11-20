import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

type AppInfo = { name: string; version: string; os: string };

function App() {
  const [info, setInfo] = useState<AppInfo | null>(null);

  useEffect(() => {
    invoke<AppInfo>("app_info").then(setInfo).catch(console.error);
  }, []);

  return (
    <main className="container">
      <h1>Plutus</h1>
      <p className="tagline">Personal finances, kept on your machine.</p>
      <p className="meta">
        {info ? `v${info.version} \u00b7 ${info.os}` : "Loading\u2026"}
      </p>
    </main>
  );
}

export default App;
