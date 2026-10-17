import type { ReactNode } from "react";

type Props = {
  /** An object URL of the image's bytes (the pane revokes it when it is replaced). */
  url: string;
  name: string;
  /** The image's pixel size, once it has loaded, for the header. */
  onSize: (width: number, height: number) => void;
  /** Something laid over the image (a region being marked); the box around the image is positioned and counts as page 1. */
  overlay?: ReactNode;
};

/** The image, fitted to the pane. */
export function ImageView({ url, name, onSize, overlay }: Props) {
  return (
    <div className="preview__imagebox" data-page="1">
      <img className="preview__image" src={url} alt={name} draggable={false} onLoad={(e) => onSize(e.currentTarget.naturalWidth, e.currentTarget.naturalHeight)} />
      {overlay}
    </div>
  );
}
