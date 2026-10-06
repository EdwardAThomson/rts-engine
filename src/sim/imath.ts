// Integer maths helpers. The sim must give identical results on every machine, so it avoids floating-point
// functions such as Math.sqrt and Math.sin, whose last bits are not guaranteed to match across engines.

/** Floor of the square root of a non-negative integer, by Newton's method on integers. */
export function isqrt(n: number): number {
  if (!Number.isSafeInteger(n) || n < 0) throw new Error(`isqrt needs a non-negative safe integer, got ${n}`);
  if (n < 2) return n;
  let x = n;
  let y = Math.floor((x + 1) / 2);
  while (y < x) {
    x = y;
    y = Math.floor((x + Math.floor(n / x)) / 2);
  }
  return x;
}
