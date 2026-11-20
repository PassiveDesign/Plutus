import { useCallback, useEffect, useState } from "react";
import { api, AppInfo, errorMessage, VaultStatus } from "./lib/api";
import { SetupScreen } from "./screens/SetupScreen";
import { UnlockScreen } from "./screens/UnlockScreen";
import "./App.css";

function App() {
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [status, setStatus] = useState<VaultStatus | null>(null);
  const [fatal, setFatal] = useState<string | null>(null);

  const refresh = useCallback(() => {
    api.vaultStatus().then(setStatus).catch((e) => setFatal(errorMessage(e)));
  }, []);

  useEffect(() => {
    api.appInfo().then(setInfo).catch((e) => setFatal(errorMessage(e)));
    refresh();
  }, [refresh]);

  async function lock() {
    await api.lockVault();
    refresh();
  }

  if (fatal) {
    return (
      <main className="auth">
        <div className="card">
          <h1>Something went wrong</h1>
          <p className="error">{fatal}</p>
        </div>
      </main>
    );
  }

  if (!info || !status) {
    return <main className="auth" aria-busy="true" />;
  }

  if (status === "new") {
    return <SetupScreen minChars={info.min_password_chars} onCreated={refresh} />;
  }

  if (status === "locked") {
    return <UnlockScreen onUnlocked={refresh} />;
  }

  // Unlocked: the real app shell replaces this in the next roadmap task.
  return (
    <main className="container">
      <h1>Plutus</h1>
      <p className="tagline">Personal finances, kept on your machine.</p>
      <button className="secondary" onClick={lock}>
        Lock
      </button>
      <p className="meta">
        v{info.version} · {info.os}
      </p>
    </main>
  );
}

export default App;
