import { type RefCallback, useEffect, useState } from "react";

/**
 * True once the element comes within `rootMargin` of the viewport, and true from then on. Without
 * IntersectionObserver it is true at once, so content still loads, only not lazily.
 */
export function useNearViewport<T extends Element>(rootMargin: string): [RefCallback<T>, boolean] {
  const [node, setNode] = useState<T | null>(null);
  const [near, setNear] = useState(() => typeof IntersectionObserver === "undefined");
  useEffect(() => {
    if (near || node === null) {
      return;
    }
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((entry) => entry.isIntersecting)) {
          observer.disconnect();
          setNear(true);
        }
      },
      { rootMargin },
    );
    observer.observe(node);
    return () => {
      observer.disconnect();
    };
  }, [near, node, rootMargin]);
  return [setNode, near];
}
