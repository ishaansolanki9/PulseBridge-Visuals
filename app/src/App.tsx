import { ControlApp } from "./control/ControlApp";
import { PerformanceOutput } from "./visuals/PerformanceOutput";

export default function App() {
  const isPerformanceOutput = new URLSearchParams(window.location.search).has("performance");
  return isPerformanceOutput ? <PerformanceOutput /> : <ControlApp />;
}
