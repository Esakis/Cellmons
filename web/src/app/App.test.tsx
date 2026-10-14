import { render, screen } from '@solidjs/testing-library';
import { describe, expect, it } from 'vitest';
import { App } from './App';

describe('App', () => {
  it('pokazuje nazwę gry w nagłówku', () => {
    render(() => <App />);
    expect(screen.getByRole('heading', { level: 1 }).textContent).toBe('Spellgambit: Arcane Class');
  });
});
