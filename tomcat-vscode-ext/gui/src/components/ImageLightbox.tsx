import { useEffect, useRef } from "react";

export interface ZoomedImage {
  alt: string;
  src: string;
}

export function ImageLightbox({
  image,
  onClose,
}: {
  image: ZoomedImage | null;
  onClose(): void;
}) {
  const closeButtonRef = useRef<HTMLButtonElement | null>(null);
  const previousFocusRef = useRef<HTMLElement | null>(null);

  useEffect(() => {
    if (!image) {
      return;
    }
    previousFocusRef.current =
      document.activeElement instanceof HTMLElement ? document.activeElement : null;
    closeButtonRef.current?.focus();
    return () => {
      const previousFocus = previousFocusRef.current;
      if (previousFocus && previousFocus.isConnected) {
        previousFocus.focus();
      }
      previousFocusRef.current = null;
    };
  }, [image]);

  useEffect(() => {
    if (!image) {
      return;
    }
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape") {
        return;
      }
      event.preventDefault();
      event.stopPropagation();
      onClose();
    };
    document.addEventListener("keydown", handleKeyDown, true);
    return () => {
      document.removeEventListener("keydown", handleKeyDown, true);
    };
  }, [image, onClose]);

  if (!image) {
    return null;
  }

  return (
    <div
      className="tc-image-lightbox__overlay"
      data-testid="image-lightbox-overlay"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) {
          onClose();
        }
      }}
      role="presentation"
    >
      <section
        aria-label="Image preview"
        aria-modal="true"
        className="tc-image-lightbox"
        data-testid="image-lightbox"
        onMouseDown={(event) => {
          if (event.target === event.currentTarget) {
            onClose();
            return;
          }
          event.stopPropagation();
        }}
        role="dialog"
      >
        <button
          aria-label="Close image preview"
          className="tc-image-lightbox__close"
          data-testid="image-lightbox-close"
          onClick={onClose}
          ref={closeButtonRef}
          type="button"
        >
          <span aria-hidden="true" className="codicon codicon-close" />
        </button>
        <img
          alt={image.alt}
          className="tc-image-lightbox__image"
          data-testid="image-lightbox-image"
          onMouseDown={(event) => {
            event.stopPropagation();
          }}
          src={image.src}
        />
      </section>
    </div>
  );
}
