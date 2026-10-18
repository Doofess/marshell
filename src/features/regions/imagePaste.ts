import { encode, fitPng } from "./crop";

type PasteData = { getData(t: string): string; files: ArrayLike<{ type: string }> };

/** The index of the first image file of a paste that carries no text, else null. A paste with text (from Word or a browser) stays a text paste. */
export function imageOnlyPaste(data: PasteData): number | null {
  if (data.getData("text/plain").trim() !== "") return null;
  for (let i = 0; i < data.files.length; i++) {
    if (data.files[i]?.type.startsWith("image/")) return i;
  }
  return null;
}

/** A PNG under the 20 MB cap: a PNG as it is, any other image drawn again as a PNG. */
export async function toPng(blob: Blob): Promise<Blob> {
  if (blob.type === "image/png") return fitPng(blob);
  const bitmap = await createImageBitmap(blob);
  try {
    const size = { w: bitmap.width, h: bitmap.height };
    return await fitPng(await encode(bitmap, { sx: 0, sy: 0, sw: size.w, sh: size.h }, size));
  } finally {
    bitmap.close();
  }
}
