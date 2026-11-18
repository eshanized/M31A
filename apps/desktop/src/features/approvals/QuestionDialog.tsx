import React, { useState } from 'react';
import { HelpCircle, Send } from 'lucide-react';
import type { PendingQuestionDto } from '../../types/ipc';
import { useApp } from '../../stores/AppContext';

export const QuestionDialog: React.FC<{ question: PendingQuestionDto }> = ({
  question,
}) => {
  const { resolveQuestion } = useApp();
  const [selectedOptions, setSelectedOptions] = useState<string[]>([]);
  const [customText, setCustomText] = useState('');
  const [isSubmitting, setIsSubmitting] = useState(false);

  const toggleOption = (opt: string) => {
    if (question.multi_select) {
      if (selectedOptions.includes(opt)) {
        setSelectedOptions(selectedOptions.filter((o) => o !== opt));
      } else {
        setSelectedOptions([...selectedOptions, opt]);
      }
    } else {
      setSelectedOptions([opt]);
    }
  };

  const handleSubmit = async () => {
    setIsSubmitting(true);
    try {
      let answers: string[] = [];
      if (question.options && question.options.length > 0) {
        answers = [...selectedOptions];
        if (customText.trim()) {
          answers.push(customText.trim());
        }
      } else {
        answers = [customText.trim()];
      }

      await resolveQuestion(question.question_id, answers);
    } catch (err) {
      console.error('Failed to submit question response:', err);
    } finally {
      setIsSubmitting(false);
    }
  };

  return (
    <div
      style={{
        margin: '12px 16px',
        padding: '12px 16px',
        borderRadius: 'var(--radius-md)',
        backgroundColor: 'rgba(31, 111, 235, 0.08)',
        border: '1px solid var(--accent-blue)',
        boxShadow: '0 4px 12px rgba(0, 0, 0, 0.2)',
      }}
    >
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          gap: '8px',
          marginBottom: '8px',
          color: 'var(--accent-blue)',
          fontWeight: 600,
          fontSize: '12px',
        }}
      >
        <HelpCircle size={16} />
        <span>Clarification Requested</span>
      </div>

      <div
        style={{
          fontSize: '13px',
          color: 'var(--text-primary)',
          fontWeight: 500,
          marginBottom: '12px',
        }}
      >
        {question.question}
      </div>

      {question.options && question.options.length > 0 ? (
        <div style={{ display: 'flex', flexDirection: 'column', gap: '6px', marginBottom: '12px' }}>
          {question.options.map((opt) => {
            const isSelected = selectedOptions.includes(opt);
            return (
              <label
                key={opt}
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  gap: '8px',
                  padding: '6px 10px',
                  borderRadius: 'var(--radius-sm)',
                  backgroundColor: isSelected ? 'rgba(56, 139, 253, 0.15)' : 'var(--bg-secondary)',
                  border: `1px solid ${isSelected ? 'var(--accent-blue)' : 'var(--border-primary)'}`,
                  cursor: 'pointer',
                  fontSize: '12px',
                }}
              >
                <input
                  type={question.multi_select ? 'checkbox' : 'radio'}
                  name={`question-${question.question_id}`}
                  checked={isSelected}
                  onChange={() => toggleOption(opt)}
                />
                <span>{opt}</span>
              </label>
            );
          })}
        </div>
      ) : (
        <div style={{ marginBottom: '12px' }}>
          <input
            type="text"
            value={customText}
            onChange={(e) => setCustomText(e.target.value)}
            placeholder="Type your response..."
            style={{
              width: '100%',
              padding: '8px 12px',
              borderRadius: 'var(--radius-sm)',
              border: '1px solid var(--border-primary)',
              backgroundColor: 'var(--bg-primary)',
              color: 'var(--text-primary)',
              fontSize: '12px',
            }}
          />
        </div>
      )}

      <div style={{ display: 'flex', justifyContent: 'flex-end' }}>
        <button
          type="button"
          onClick={handleSubmit}
          disabled={isSubmitting || (question.options.length > 0 && selectedOptions.length === 0 && !customText.trim())}
          className="btn btn-primary btn-sm"
        >
          <Send size={12} /> Submit Response
        </button>
      </div>
    </div>
  );
};
