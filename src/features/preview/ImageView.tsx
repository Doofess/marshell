type Props = {
  /** An object URL of the image's bytes (the pane revokes it when it is replaced). */
  url: string;
  name: string;
  /** The image's pixel size, once it has loaded, for the header. */
  onSize: (width: number, height: number) => void;
};

/** The image, fitted to the pane. */
export function ImageView({ url, name, onSize }: Props) {
  return <img className="preview__image" src={url} alt={name} draggable={false} onLoad={(e) => onSize(e.currentTarget.naturalWidth, e.currentTarget.naturalHeight)} />;
}
