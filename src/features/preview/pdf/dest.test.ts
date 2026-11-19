import { describe, expect, it } from "vitest";
import type { PDFDocumentProxy } from "pdfjs-dist";
import { placeOfDest, topOf } from "./dest";

// A page 800 high: the PDF's y runs up from the bottom, the view's down from the top.
const toViewY = (y: number) => 800 - y;

describe("topOf", () => {
  it("topOf_xyz_takes_the_second_argument", () => {
    expect(topOf([{ name: "XYZ" }, 10, 600, null], toViewY, 800)).toBe(0.25);
  });
  it("topOf_fith_and_fitbh_take_the_first", () => {
    expect(topOf([{ name: "FitH" }, 400], toViewY, 800)).toBe(0.5);
    expect(topOf([{ name: "FitBH" }, 400], toViewY, 800)).toBe(0.5);
  });
  it("topOf_fit_gives_the_page_top", () => {
    expect(topOf([{ name: "Fit" }], toViewY, 800)).toBe(0);
  });
  it("topOf_a_null_top_gives_0", () => {
    expect(topOf([{ name: "XYZ" }, null, null, null], toViewY, 800)).toBe(0);
    expect(topOf([{ name: "FitH" }, null], toViewY, 800)).toBe(0);
  });
  it("topOf_is_clamped_to_the_page", () => {
    expect(topOf([{ name: "FitH" }, 5000], toViewY, 800)).toBe(0);
    expect(topOf([{ name: "FitH" }, -5000], toViewY, 800)).toBe(1);
    expect(topOf([{ name: "FitH" }, 400], toViewY, 0)).toBe(0);
  });
});

const ref = { num: 7, gen: 0 };
function fakeDoc(over: Partial<Record<"getDestination" | "getPageIndex", (x: never) => Promise<unknown>>> = {}) {
  const page = { getViewport: () => ({ height: 800, convertToViewportPoint: (_x: number, y: number) => [0, 800 - y] }) };
  return {
    numPages: 3,
    getDestination: async (name: string) => (name === "chap" ? [ref, { name: "XYZ" }, 0, 600, null] : null),
    getPageIndex: async (r: unknown) => {
      if (r === ref) return 1;
      throw new Error("no such page");
    },
    getPage: async () => page,
    ...over,
  } as unknown as PDFDocumentProxy;
}

describe("placeOfDest", () => {
  it("placeOfDest_a_named_destination", async () => {
    expect(await placeOfDest(fakeDoc(), "chap")).toEqual({ page: 2, top: 0.25 });
  });
  it("placeOfDest_a_missing_name_is_null", async () => {
    expect(await placeOfDest(fakeDoc(), "nope")).toBeNull();
  });
  it("placeOfDest_a_page_reference", async () => {
    expect(await placeOfDest(fakeDoc(), [ref, { name: "Fit" }])).toEqual({ page: 2, top: 0 });
  });
  it("placeOfDest_a_bare_page_number", async () => {
    expect(await placeOfDest(fakeDoc(), [2, { name: "FitH" }, 400])).toEqual({ page: 3, top: 0.5 });
  });
  it("placeOfDest_a_page_out_of_range_is_null", async () => {
    expect(await placeOfDest(fakeDoc(), [3, { name: "Fit" }])).toBeNull();
    expect(await placeOfDest(fakeDoc(), [-1, { name: "Fit" }])).toBeNull();
    expect(await placeOfDest(fakeDoc(), [{ num: 99, gen: 0 }, { name: "Fit" }])).toBeNull();
  });
  it("placeOfDest_a_null_destination_is_null", async () => {
    expect(await placeOfDest(fakeDoc(), [])).toBeNull();
    expect(await placeOfDest(fakeDoc(), [null, { name: "Fit" }])).toBeNull();
    expect(await placeOfDest(fakeDoc({ getDestination: async () => { throw new Error("x"); } }), "chap")).toBeNull();
  });
});
