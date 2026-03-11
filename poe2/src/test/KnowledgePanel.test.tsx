import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import './mocks/tauri';
import { mockInvoke } from './mocks/tauri';
import { makeKnowledgeEntry } from './factories';
import KnowledgePanel from '../components/KnowledgePanel';

describe('KnowledgePanel', () => {
  beforeEach(() => {
    mockInvoke.mockResolvedValue(undefined);
  });

  it('renders knowledge entries', () => {
    const entries = [
      makeKnowledgeEntry({ id: 'e1', key: 'project-goal', value: 'Build a great product.' }),
      makeKnowledgeEntry({ id: 'e2', key: 'tech-stack', value: 'React + Tauri + Rust' }),
    ];
    render(<KnowledgePanel entries={entries} projectId="p1" onClose={vi.fn()} />);
    expect(screen.getByText('project-goal')).toBeInTheDocument();
    expect(screen.getByText('tech-stack')).toBeInTheDocument();
  });

  it('renders empty state when no entries', () => {
    render(<KnowledgePanel entries={[]} projectId="p1" onClose={vi.fn()} />);
    expect(screen.getByText('No entries')).toBeInTheDocument();
  });

  it('renders entry value in truncated form in list', () => {
    const entry = makeKnowledgeEntry({ key: 'summary', value: 'This is a concise summary.' });
    render(<KnowledgePanel entries={[entry]} projectId="p1" onClose={vi.fn()} />);
    expect(screen.getByText('This is a concise summary.')).toBeInTheDocument();
  });

  it('Close button calls onClose', () => {
    const onClose = vi.fn();
    render(<KnowledgePanel entries={[]} projectId="p1" onClose={onClose} />);
    const closeBtn = screen.getByText('✕ close');
    fireEvent.click(closeBtn);
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('shows footer entry count', () => {
    const entries = [
      makeKnowledgeEntry({ id: 'e1' }),
      makeKnowledgeEntry({ id: 'e2' }),
    ];
    render(<KnowledgePanel entries={entries} projectId="p1" onClose={vi.fn()} />);
    expect(screen.getByText('2 entries')).toBeInTheDocument();
  });
});
