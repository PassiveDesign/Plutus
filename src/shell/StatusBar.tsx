type Props = { version: string };

/** The bar along the bottom: how things stand, in mono. */
export function StatusBar({ version }: Props) {
  return (
    <footer className="status">
      <span className="mode">Local</span>
      <span>Unlocked</span>
      <span>Encrypted, on this computer only</span>
      <span className="keys" aria-hidden="true">
        <span>
          <b>Ctrl L</b> lock
        </span>
      </span>
      <span className="foot">Plutus v{version}</span>
    </footer>
  );
}
