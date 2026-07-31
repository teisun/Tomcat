import {
  CUSTOM_OPTION_ID,
  type AskQuestionAnswer,
  type AskQuestionOutcome,
  type AskQuestionResult,
  type WebviewApprovalOption,
  type WebviewApprovalQuestion,
} from "../types";

function buildOptionCode(index: number): string {
  let remaining = index;
  let code = "";
  do {
    code = String.fromCharCode(65 + (remaining % 26)) + code;
    remaining = Math.floor(remaining / 26) - 1;
  } while (remaining >= 0);
  return code;
}

function effectiveOutcome(result: AskQuestionResult): AskQuestionOutcome {
  return result.outcome ?? (result.cancelled ? "cancelled_unknown" : "answered");
}

function outcomeLabel(outcome: AskQuestionOutcome): string {
  switch (outcome) {
    case "answered":
      return "Answered";
    case "skipped":
      return "Entire request skipped";
    case "interrupted":
      return "Interrupted";
    case "host_disconnected":
      return "Disconnected";
    case "cancelled_unknown":
      return "Cancelled (legacy result)";
  }
}

function questionStatus(
  outcome: AskQuestionOutcome,
  answer: AskQuestionAnswer | undefined,
): string {
  if (outcome !== "answered") return outcomeLabel(outcome);
  if (!answer) return "Not answered";
  if (answer.skipped || answer.optionIds.length === 0) return "Skipped";
  return "Answered";
}

function optionIsSelected(answer: AskQuestionAnswer | undefined, optionId: string): boolean {
  return !!answer && !answer.skipped && answer.optionIds.includes(optionId);
}

function optionLabel(
  option: WebviewApprovalOption,
  answer: AskQuestionAnswer | undefined,
): string {
  if (option.id !== CUSTOM_OPTION_ID || !optionIsSelected(answer, CUSTOM_OPTION_ID)) {
    return option.label;
  }
  const custom = answer?.customText?.trim();
  return custom ? `Other — ${custom}` : "Other";
}

export function AnswerCard({
  questions,
  result,
}: {
  questions: WebviewApprovalQuestion[];
  result: AskQuestionResult;
}) {
  const outcome = effectiveOutcome(result);
  const answersByQuestion = new Map(result.answers.map((answer) => [answer.questionId, answer]));
  const answeredCount = questions.filter((question) => {
    const answer = answersByQuestion.get(question.id);
    return !!answer && !answer.skipped && answer.optionIds.length > 0;
  }).length;
  const skippedCount = questions.filter((question) => {
    const answer = answersByQuestion.get(question.id);
    return !!answer && (answer.skipped || answer.optionIds.length === 0);
  }).length;
  const summary =
    outcome === "answered"
      ? `${answeredCount} answered${skippedCount ? ` · ${skippedCount} skipped` : ""}`
      : outcomeLabel(outcome);

  return (
    <section
      aria-label={`Question result: ${outcomeLabel(outcome)}`}
      className={`tc-card tc-answer-card tc-answer-card--${outcome}`}
      data-outcome={outcome}
      data-testid="answer-card"
    >
      <div className="tc-card__header">
        <h3>Answers</h3>
        <span className="tc-chip" data-testid="answer-card-outcome">{summary}</span>
      </div>
      <div className="tc-answer-card__questions">
        {questions.map((question, questionIndex) => {
          const answer = answersByQuestion.get(question.id);
          const status = questionStatus(outcome, answer);
          const options: WebviewApprovalOption[] = [
            ...question.options,
            { id: CUSTOM_OPTION_ID, label: "Other" },
          ];

          return (
            <div className="tc-approval-question" data-testid="answer-card-question" key={question.id}>
              <div className="tc-approval-question__prompt">
                <span className="tc-approval-question__index">{questionIndex + 1}.</span>
                <p>{question.prompt}</p>
                <span className="tc-answer-card__status" data-testid={`answer-status-${question.id}`}>
                  {status}
                </span>
              </div>
              <div
                aria-label={`${question.prompt}: ${status}`}
                className="tc-approval-options tc-answer-card__options"
                role="list"
              >
                {options.map((option, optionIndex) => {
                  const selected = outcome === "answered" && optionIsSelected(answer, option.id);
                  return (
                    <div
                      aria-current={selected ? "true" : undefined}
                      className={
                        selected
                          ? "tc-approval-option tc-approval-option--selected tc-answer-card__option"
                          : "tc-approval-option tc-answer-card__option tc-answer-card__option--unselected"
                      }
                      data-option-id={option.id}
                      data-testid={
                        selected
                          ? `answer-option-${question.id}`
                          : `answer-option-${question.id}-${option.id}`
                      }
                      key={option.id}
                      role="listitem"
                    >
                      <span
                        aria-hidden="true"
                        className={
                          selected
                            ? "tc-approval-option__code tc-approval-option__code--selected"
                            : "tc-approval-option__code"
                        }
                      >
                        {buildOptionCode(optionIndex)}
                      </span>
                      <span className="tc-approval-option__content">
                        <span className="tc-approval-option__label">{optionLabel(option, answer)}</span>
                        {option.recommended ? (
                          <span className="tc-approval-option__recommended">Recommended</span>
                        ) : null}
                      </span>
                    </div>
                  );
                })}
              </div>
            </div>
          );
        })}
      </div>
    </section>
  );
}
