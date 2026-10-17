import { useState, type CSSProperties, type ReactNode } from "react";

type Props = {
  /** An object URL of the image's bytes (the pane revokes it when it is replaced). */
  url: string;
  name: string;
  /** The image's pixel size, once it has loaded, for the header. */
  onSize: (width: number, height: number) => void;
  /** Something laid over the image (a region being marked); the box around the image is positioned and counts as page 1. */
  overlay?: ReactNode;
};

type Natural = { url: string; w: number; h: number };

/** The loaded size as the two numbers Preview.css fits the box with (null before the image has loaded). */
export function imageBoxStyle(nat: { w: number; h: number } | null): CSSProperties | undefined {
  return nat === null ? undefined : ({ "--nat-w": nat.w, "--nat-h": nat.h } as CSSProperties);
}

/**
 * The image, fitted to the pane. The box around it (data-page="1") is exactly as big as the image is drawn, so a region's
 * fractions of the box are the same fractions of the image. Once the image has loaded, the box is sized from its natural
 * size and the frame it sits in (Preview.css); the image fills the box.
 */
export function ImageView({ url, name, onSize, overlay }: Props) {
  const [nat, setNat] = useState<Natural | null>(null);
  const known = nat !== null && nat.url === url && nat.w > 0 && nat.h > 0 ? nat : null;
  return (
    <div className="preview__imagefit">
      <div className="preview__imagebox" data-page="1" data-fit={known ? "true" : undefined} style={imageBoxStyle(known)}>
        <img
          className="preview__image"
          src={url}
          alt={name}
          draggable={false}
          onLoad={(e) => {
            const { naturalWidth: w, naturalHeight: h } = e.currentTarget;
            setNat({ url, w, h });
            onSize(w, h);
          }}
        />
        {overlay}
      </div>
    </div>
  );
}
