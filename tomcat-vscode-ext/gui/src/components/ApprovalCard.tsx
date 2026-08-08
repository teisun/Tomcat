import { memo } from "react";

import {
  CUSTOM_OPTION_ID,
  type AskQuestionResult,
  type WebviewApprovalCard,
  type WebviewApprovalOption,
} from "../types";

export type ApprovalQuestionDraft = {
  customText: string;
  optionId: string | null;
};

export type ApprovalAnswerDraft = Record<string, ApprovalQuestionDraft>;

export type ApprovalAnswerState = {
  draft: ApprovalAnswerDraft;
  submitting: boolean;
};

export function approvalAnswerKey(sessionId: string, requestId: string): string {
  return `${sessionId}\u0000${requestId}`;
}

export function createApprovalAnswerDraft(item: WebviewApprovalCard): ApprovalAnswerDraft {
  return Object.fromEntries(
    item.request.questions.map((question) => [
      question.id,
      {
        customText: "",
        optionId: null,
      },
    ]),
  );
}

function buildOptionCode(index: number): string {
  let remaining = index;
  let code = "";

  do {
    code = String.fromCharCode(65 + (remaining % 26)) + code;
    remaining = Math.floor(remaining / 26) - 1;
  } while (remaining >= 0);

  return code;
}

function ApprovalCardComponent({
  draft: suppliedDraft,
  item,
  onAnswer,
  onDraftChange,
  submitting = false,
}: {
  draft?: ApprovalAnswerDraft;
  item: WebviewApprovalCard;
  onAnswer(sessionId: string, requestId: string, result: AskQuestionResult): void;
  onDraftChange(sessionId: string, requestId: string, draft: ApprovalAnswerDraft): void;
  submitting?: boolean;
}) {
  const draft = suppliedDraft ?? createApprovalAnswerDraft(item);
  if (item.resolved) {
    return null;
  }
  if (!item.live) {
    return (
      <section
        aria-live="polite"
        className="tc-card tc-approval-card tc-approval-card--restoring"
        data-testid="approval-card-restoring"
      >
        <div className="tc-card__header">
          <h3>Question pending</h3>
        </div>
        <p>Restoring this question…</p>
      </section>
    );
  }

  const canContinue = item.request.questions.every((question) => {
    const questionDraft = draft[question.id];
    if (!questionDraft?.optionId) {
      return false;
    }
    if (questionDraft.optionId !== CUSTOM_OPTION_ID) {
      return true;
    }
    return questionDraft.customText.trim().length > 0;
  });

  const selectOption = (questionId: string, optionId: string) => {
    if (submitting) return;
    const current = draft[questionId] ?? { customText: "", optionId: null };
    onDraftChange(item.sessionId ?? "", item.request.requestId, {
      ...draft,
      [questionId]: {
        ...current,
        optionId,
      },
    });
  };

  const updateCustomText = (questionId: string, customText: string) => {
    if (submitting) return;
    const current = draft[questionId] ?? { customText: "", optionId: CUSTOM_OPTION_ID };
    onDraftChange(item.sessionId ?? "", item.request.requestId, {
      ...draft,
      [questionId]: {
        ...current,
        customText,
      },
    });
  };

  const submitAnswers = () => {
    if (!canContinue || submitting) {
      return;
    }

    onAnswer(item.sessionId ?? "", item.request.requestId, {
      answers: item.request.questions.map((question) => {
        const questionDraft = draft[question.id];
        const optionId = questionDraft?.optionId;
        if (!optionId) {
          throw new Error(`missing approval answer for question ${question.id}`);
        }
        if (optionId === CUSTOM_OPTION_ID) {
          return {
            customText: questionDraft.customText.trim(),
            optionIds: [CUSTOM_OPTION_ID],
            pickedRecommended: false,
            questionId: question.id,
          };
        }

        const selectedOption = question.options.find((option) => option.id === optionId);
        return {
          optionIds: [optionId],
          pickedRecommended: !!selectedOption?.recommended,
          questionId: question.id,
        };
      }),
      cancelled: false,
      outcome: "answered",
    });
  };

  const skipQuestions = () => {
    if (submitting) return;
    onAnswer(item.sessionId ?? "", item.request.requestId, {
      answers: [],
      cancelled: true,
      outcome: "skipped",
    });
  };

  return (
    <section className="tc-card tc-approval-card" data-testid="approval-card">
      <div className="tc-card__header">
        <h3>Questions</h3>
        <span className="tc-chip tc-chip--warning">{item.request.questions.length} of {item.request.questions.length}</span>
      </div>
      <div className="tc-approval-questions">
        {item.request.questions.map((question, questionIndex) => {
          const questionDraft = draft[question.id] ?? { customText: "", optionId: null };
          const options: WebviewApprovalOption[] = [
            ...question.options,
            { id: CUSTOM_OPTION_ID, label: "Other..." },
          ];

          return (
            <div className="tc-approval-question" key={question.id}>
              <div className="tc-approval-question__prompt">
                <span className="tc-approval-question__index">{questionIndex + 1}.</span>
                <p>{question.prompt}</p>
              </div>
              <div
                aria-label={question.prompt}
                className="tc-approval-options"
                role="radiogroup"
              >
                {options.map((option, optionIndex) => {
                  const selected = questionDraft.optionId === option.id;
                  return (
                    <button
                      aria-checked={selected}
                      className={
                        selected
                          ? "tc-approval-option tc-approval-option--selected"
                          : "tc-approval-option"
                      }
                      data-testid={`approval-option-${question.id}-${option.id}`}
                      disabled={submitting}
                      key={option.id}
                      onClick={() => selectOption(question.id, option.id)}
                      role="radio"
                      type="button"
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
                        <span className="tc-approval-option__label">{option.label}</span>
                        {option.recommended ? (
                          <span className="tc-approval-option__recommended">Recommended</span>
                        ) : null}
                      </span>
                    </button>
                  );
                })}
              </div>
              {questionDraft.optionId === CUSTOM_OPTION_ID ? (
                <label className="tc-field tc-approval-custom">
                  <span>Custom answer</span>
                  <input
                    className="tc-approval-custom__input"
                    data-testid={`approval-custom-${question.id}`}
                    disabled={submitting}
                    onChange={(event) => updateCustomText(question.id, event.target.value)}
                    placeholder="Enter a custom answer"
                    type="text"
                    value={questionDraft.customText}
                  />
                </label>
              ) : null}
            </div>
          );
        })}
      </div>
      <div className="tc-approval-actions">
        <button
          className="tc-button tc-button--ghost"
          data-testid="approval-skip"
          disabled={submitting}
          onClick={skipQuestions}
          type="button"
        >
          Skip
        </button>
        <button
          className="tc-button tc-button--primary"
          data-testid="approval-continue"
          disabled={!canContinue || submitting}
          onClick={submitAnswers}
          type="button"
        >
          {submitting ? "Submitting…" : "Continue"}
        </button>
      </div>
    </section>
  );
}

function areApprovalCardPropsEqual(
  previous: Readonly<Parameters<typeof ApprovalCardComponent>[0]>,
  next: Readonly<Parameters<typeof ApprovalCardComponent>[0]>,
): boolean {
  return (
    previous.item === next.item &&
    previous.draft === next.draft &&
    previous.submitting === next.submitting &&
    previous.onAnswer === next.onAnswer &&
    previous.onDraftChange === next.onDraftChange
  );
}

export const ApprovalCard = memo(ApprovalCardComponent, areApprovalCardPropsEqual);
