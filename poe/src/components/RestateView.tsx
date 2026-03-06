import { useTheme } from "../hooks/useTheme";

const RESTATE_ADMIN_URL = "http://localhost:9070";

export function RestateView() {
  const { isDark } = useTheme();

  return (
    <div className="flex-1 flex flex-col overflow-hidden">
      <iframe
        src={RESTATE_ADMIN_URL}
        className="flex-1 w-full border-none"
        title="Restate Admin"
        sandbox="allow-same-origin allow-scripts allow-forms allow-popups"
        style={isDark ? { filter: "invert(1) hue-rotate(180deg)" } : undefined}
      />
    </div>
  );
}
