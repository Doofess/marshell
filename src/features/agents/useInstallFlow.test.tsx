import { renderToStaticMarkup } from "react-dom/server";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { applyPreview, planInstall } from "../../lib/api/agents";
import { PREVIEW } from "./fixtures";
import { useInstallFlow } from "./useInstallFlow";

vi.mock("../../lib/api/agents", () => ({ planInstall: vi.fn(), applyPreview: vi.fn() }));
const endpoint = { port: 9, token: "test", platform: "windows" as const };

function flow() {
  let result!: ReturnType<typeof useInstallFlow>;
  function Harness() {
    result = useInstallFlow(endpoint, vi.fn());
    return null;
  }
  renderToStaticMarkup(<Harness />);
  return result;
}

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(planInstall).mockResolvedValue(PREVIEW);
});

describe("useInstallFlow requests", () => {
  it("plans approval by default and replans when its switch changes without applying", async () => {
    const install = flow();
    await install.plan();
    expect(planInstall).toHaveBeenLastCalledWith(endpoint, "claude", { status: true, statusline: true, approve: true });
    install.choose("approve", false);
    expect(planInstall).toHaveBeenLastCalledWith(endpoint, "claude", { status: true, statusline: true, approve: false });
    expect(applyPreview).not.toHaveBeenCalled();
  });

  it("keeps an approval-only upgrade through toggles and reload", async () => {
    const install = flow();
    await install.plan({ status: false, statusline: false, approve: true });
    expect(planInstall).toHaveBeenLastCalledWith(endpoint, "claude", { status: false, statusline: false, approve: true });
    install.choose("approve", false);
    await install.plan();
    expect(planInstall).toHaveBeenLastCalledWith(endpoint, "claude", { status: false, statusline: false, approve: false });
    expect(applyPreview).not.toHaveBeenCalled();
  });
});
