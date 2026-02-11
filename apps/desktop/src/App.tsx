import type { ProviderId } from "@agentdock/contracts/provider";
import "./App.css";

const supportedProviders: ProviderId[] = ["codex", "claude_code"];

function App() {
  return (
    <main className="container">
      <h1>AgentDock</h1>
      <p>Desktop control plane shell is ready.</p>
      <p>Supported providers: {supportedProviders.join(", ")}</p>
    </main>
  );
}

export default App;
