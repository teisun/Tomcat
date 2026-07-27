import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { PlanBuildModelSelect } from "./PlanBuildModelSelect";

function selectEl(): HTMLSelectElement {
  return screen.getByTestId("plan-build-model-select") as HTMLSelectElement;
}

describe("PlanBuildModelSelect", () => {
  it("lists only concrete models and reflects the configured value", () => {
    render(
      <PlanBuildModelSelect
        availableModels={["gpt-5.6", "claude-opus"]}
        onChange={() => undefined}
        sessionModel="gpt-5.6"
        value="claude-opus"
      />,
    );
    const select = selectEl();
    expect(select.value).toBe("claude-opus");
    // 没有空值选项：一个间接引用在这个扁平样式里和具体模型名长得一模一样，
    // 分不出「默认」和「别人几周前设过的值」，事故就是这么来的。
    expect(Array.from(select.options).map((option) => option.value)).toEqual([
      "gpt-5.6",
      "claude-opus",
    ]);
  });

  it("shows the session model when the build model config is empty", () => {
    const onChange = vi.fn();
    const { rerender } = render(
      <PlanBuildModelSelect
        availableModels={["gpt-5.6", "claude-opus"]}
        onChange={onChange}
        sessionModel="gpt-5.6"
        value=""
      />,
    );
    expect(selectEl().value).toBe("gpt-5.6");
    // 只是显示，不写回配置 —— 否则一个瞬时值会被固化成全局设置。
    expect(onChange).not.toHaveBeenCalled();

    // 会话模型换了，显示值跟着换：所见即所跑。
    rerender(
      <PlanBuildModelSelect
        availableModels={["gpt-5.6", "claude-opus"]}
        onChange={onChange}
        sessionModel="claude-opus"
        value=""
      />,
    );
    expect(selectEl().value).toBe("claude-opus");
    expect(onChange).not.toHaveBeenCalled();
  });

  it("invokes onChange with the selected model id", () => {
    const onChange = vi.fn();
    render(
      <PlanBuildModelSelect
        availableModels={["gpt-5.6", "claude-opus"]}
        onChange={onChange}
        sessionModel="claude-opus"
        value=""
      />,
    );
    fireEvent.change(selectEl(), { target: { value: "gpt-5.6" } });
    expect(onChange).toHaveBeenCalledWith("gpt-5.6");
  });

  it("still shows a configured model that dropped off the ready list", () => {
    render(
      <PlanBuildModelSelect
        availableModels={["gpt-5.6"]}
        onChange={() => undefined}
        sessionModel="gpt-5.6"
        value="stale-model"
      />,
    );
    // 藏起来会让下拉框显示的模型和真正会跑的模型对不上，那才是危险的。
    expect(selectEl().value).toBe("stale-model");
    expect(Array.from(selectEl().options).map((option) => option.value)).toEqual([
      "stale-model",
      "gpt-5.6",
    ]);
  });

  it("disables the dropdown when there are no models at all", () => {
    render(
      <PlanBuildModelSelect
        availableModels={[]}
        onChange={() => undefined}
        sessionModel=""
        value=""
      />,
    );
    expect(selectEl().disabled).toBe(true);
    expect(screen.getByText("No ready models")).toBeTruthy();
  });

  it("renders only the select (no visible text label) but keeps an accessible name", () => {
    render(
      <PlanBuildModelSelect
        availableModels={["gpt-5.6"]}
        label="Build model"
        onChange={() => undefined}
        sessionModel="gpt-5.6"
        value=""
      />,
    );
    expect(screen.getByLabelText("Build model")).toBeTruthy();
    expect(document.querySelector(".tc-plan-model-select__label")).toBeNull();
    expect(screen.queryByText("Build model")).toBeNull();
  });
});
