import { AppearanceProvider } from "./features/appearance/AppearanceProvider";
import { LiveApp } from "./features/sessions/LiveApp";

export function App() {
  return (
    <AppearanceProvider>
      <LiveApp />
    </AppearanceProvider>
  );
}
