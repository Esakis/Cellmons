import { type ParentProps, Show } from 'solid-js';

// A place's painting in a gilt frame (art/paintings/scenes, ADR-021); the children are its heading,
// styled as a name plaque on the frame's lower edge (class "plaque").
export function Scene(props: ParentProps<{ src: string | undefined; about?: string }>) {
  return (
    <section class="scene">
      <div class="scene-frame">
        <img src={props.src} alt="" width="960" height="540" />
      </div>
      {props.children}
      <Show when={props.about}>
        <p>{props.about}</p>
      </Show>
    </section>
  );
}
