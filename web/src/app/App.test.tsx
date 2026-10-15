import { render, screen } from '@solidjs/testing-library';
import { describe, expect, it } from 'vitest';
import { setLanguage } from '../i18n';
import { App } from './App';
import { content } from './content';

describe('App', () => {
  it('pokazuje postacie z treści i czterech botów', () => {
    setLanguage('pl');
    render(() => <App />);
    expect(screen.getByRole('heading', { level: 1 }).textContent).toBe('Twój ruch. Twoja magia.');
    expect(screen.getAllByRole('button', { pressed: false }).length).toBe(
      content.avatars.length + 2,
    );
    expect(screen.getByRole('link', { name: /Rozpocznij trening/ }).getAttribute('href')).toContain(
      'avatar=1',
    );
  });
});
