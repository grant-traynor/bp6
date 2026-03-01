import React from 'react';

const SettingsView: React.FC = () => {
  return (
    <div className="flex-1 flex flex-col overflow-hidden bg-[var(--background-primary)]">
      <div className="px-8 py-10 max-w-4xl mx-auto w-full">
        <div className="flex items-center gap-4 mb-12">
          <div className="p-4 rounded-2xl bg-indigo-500/10 border-2 border-indigo-500/20 shadow-inner">
            <svg xmlns="http://www.w3.org/2000/svg" width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round" className="text-indigo-600 dark:text-indigo-400">
              <path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z"/>
              <circle cx="12" cy="12" r="3"/>
            </svg>
          </div>
          <div>
            <h1 className="text-4xl font-black text-[var(--text-primary)] tracking-tight">Settings</h1>
            <p className="text-[var(--text-muted)] font-medium">Configure BERT and manage AI persona templates</p>
          </div>
        </div>

        <div className="grid grid-cols-1 md:grid-cols-2 gap-8">
          <div className="p-8 rounded-3xl bg-[var(--background-secondary)] border-2 border-[var(--border-primary)] shadow-xl group hover:border-indigo-500/30 transition-all duration-300">
            <h2 className="text-sm font-black text-indigo-600 dark:text-indigo-400 uppercase tracking-[0.25em] mb-4">General Configuration</h2>
            <div className="space-y-6">
              <p className="text-[var(--text-muted)] text-sm font-medium leading-relaxed">
                Choose your default CLI backend and configure model complexity rules for different task levels.
              </p>
              <div className="h-px w-full bg-[var(--border-primary)]/50" />
              <div className="flex items-center justify-between p-4 rounded-xl bg-[var(--background-primary)] border-2 border-[var(--border-primary)] border-dashed opacity-50 select-none">
                <span className="text-xs font-black uppercase tracking-widest text-[var(--text-muted)]">Configuration interface coming soon</span>
              </div>
            </div>
          </div>

          <div className="p-8 rounded-3xl bg-[var(--background-secondary)] border-2 border-[var(--border-primary)] shadow-xl group hover:border-purple-500/30 transition-all duration-300">
            <h2 className="text-sm font-black text-purple-600 dark:text-purple-400 uppercase tracking-[0.25em] mb-4">Persona Management</h2>
            <div className="space-y-6">
              <p className="text-[var(--text-muted)] text-sm font-medium leading-relaxed">
                Manage your agent persona templates. Edit identity prompts and task-specific instructions.
              </p>
              <div className="h-px w-full bg-[var(--border-primary)]/50" />
              <div className="flex items-center justify-between p-4 rounded-xl bg-[var(--background-primary)] border-2 border-[var(--border-primary)] border-dashed opacity-50 select-none">
                <span className="text-xs font-black uppercase tracking-widest text-[var(--text-muted)]">Template editor coming soon</span>
              </div>
            </div>
          </div>
        </div>

        <div className="mt-12 p-10 rounded-3xl bg-indigo-600/5 border-2 border-indigo-600/10 shadow-lg text-center">
          <p className="text-[var(--text-muted)] font-medium italic">
            "Better tools lead to better architecture, and better architecture leads to better code."
          </p>
        </div>
      </div>
    </div>
  );
};

export default SettingsView;
