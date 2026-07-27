/**
 * Compact build-model dropdown shared by the plan preview action strip and the
 * chat PlanFileCard. Cursor-flat: no visible text label, just the borderless
 * native select. `label` survives only as the `aria-label` for accessibility
 * and tests.
 *
 * There is deliberately no "Session default" option: an indirect value renders
 * exactly like a concrete model name in this flat style, so you cannot tell at
 * a glance whether you are looking at the default or at something someone set
 * weeks ago in another window. The dropdown always shows the model this build
 * would actually use — the configured `tomcat.plan.buildModel` when set, the
 * session's own model otherwise. Falling back is display-only; nothing is
 * written back to the global config until you pick an entry yourself.
 */
export function PlanBuildModelSelect({
  availableModels,
  disabled = false,
  label = "Build model",
  onChange,
  sessionModel = "",
  testId = "plan-build-model-select",
  value,
}: {
  availableModels: string[];
  disabled?: boolean;
  label?: string;
  onChange(modelId: string): void;
  sessionModel?: string;
  testId?: string;
  value: string;
}) {
  const effective = value || sessionModel;
  // The effective model may not be in the ready list (stale config, a model that
  // went away). Show it anyway — hiding it would put the select's displayed value
  // out of step with the model the build actually runs on.
  const options = effective && !availableModels.includes(effective)
    ? [effective, ...availableModels]
    : availableModels;
  const hasOptions = options.length > 0;
  return (
    <label className="tc-field tc-field--compact tc-field--dropdown tc-field--model tc-plan-model-select">
      <select
        aria-label={label}
        data-testid={testId}
        disabled={disabled || !hasOptions}
        onChange={(event) => onChange(event.target.value)}
        value={hasOptions ? effective : ""}
      >
        {hasOptions ? null : <option value="">No ready models</option>}
        {options.map((model) => (
          <option key={model} value={model}>
            {model}
          </option>
        ))}
      </select>
    </label>
  );
}
