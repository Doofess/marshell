import { placeMenu } from "./menuKeys";

type Size = { w: number; h: number };

/** Beside the menu on its right, or on its left when that would leave the window; level with the item, kept on screen. */
export function submenuPlace(menu: { left: number; right: number }, itemTop: number, size: Size, viewport: Size): { x: number; y: number } {
  const right = menu.right + 2;
  const x = right + size.w + 8 <= viewport.w ? right : menu.left - size.w - 2;
  return placeMenu({ x, y: itemTop }, size, viewport);
}
