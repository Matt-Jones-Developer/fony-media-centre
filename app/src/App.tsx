import MpvDevScreen from "./dev/MpvDevScreen";
import { useLabelStyle } from "./settings/useLabelStyle";

export default function App() {
  useLabelStyle();
  return <MpvDevScreen />;
}
