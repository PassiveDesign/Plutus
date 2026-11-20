import { useCallback, useEffect, useState } from "react";
import { api, AppInfo, errorMessage, VaultStatus } from "./lib/api";
import { SetupScreen } from "./screens/SetupScreen";
import { UnlockScreen } from "./screens/UnlockScreen";
import { AppShell } from "./shell/AppShell";
import { useToast } from "./components";
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

  const toast = useToast();

  const lock = useCallback(async () => {
    try {
      await api.lockVault();
    } catch (e) {
      toast.error(`Couldn't lock: ${errorMessage(e)}`);
    }
    refresh();
  }, [refresh, toast]);

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

  return <AppShell version={info.version} onLock={lock} />;
}

export default App;
