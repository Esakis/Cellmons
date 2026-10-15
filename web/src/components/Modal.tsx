import { createUniqueId, onCleanup, onMount, type ParentProps } from 'solid-js';
import { t } from '../i18n';

export function Modal(props: ParentProps<{ title: string; close: () => void; wide?: boolean }>) {
  let dialog!: HTMLDialogElement;
  const id = createUniqueId();
  const previous = document.activeElement as HTMLElement | null;
  onMount(() => {
    dialog.showModal();
  });
  onCleanup(() => {
    dialog.close();
    previous?.focus();
  });
  return (
    <dialog
      ref={dialog}
      class={props.wide ? 'modal-wide' : undefined}
      aria-labelledby={id}
      onCancel={(event) => {
        event.preventDefault();
        props.close();
      }}
    >
      <div class="modal-heading">
        <h2 id={id}>{props.title}</h2>
        <button type="button" class="quiet" onClick={props.close} aria-label={t('close')}>
          ×
        </button>
      </div>
      {props.children}
    </dialog>
  );
}
