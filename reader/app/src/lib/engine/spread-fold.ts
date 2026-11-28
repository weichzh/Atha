// SPDX-License-Identifier: AGPL-3.0-only
// Adapted from Readest, apps/readest-app/src/utils/pageCurl.ts (c3a95ba7).
// Copyright (c) the Readest contributors. See THIRD_PARTY_NOTICES.md.
import type { CurlPose } from "./webgl-page-curl.ts";

export interface SpreadLeaf {
  left: number;
  top: number;
  width: number;
  height: number;
  direction: 1 | -1;
}

/** In Readest's two-column model the radius tightens to zero at the spine. */
export function spreadFold(leaf: SpreadLeaf, progress: number, y: number): CurlPose {
  const grabY = Math.max(0, Math.min(1, (y - leaf.top) / leaf.height));
  const tilt = (grabY - 0.5) * 1.8 * (1 - progress);
  const length = Math.hypot(1, tilt);
  const axisX = leaf.direction / length;
  const axisY = tilt / length;
  const outerEdge = leaf.direction > 0 ? leaf.left + leaf.width : leaf.left;
  return {
    originX: outerEdge - axisX * leaf.width * progress,
    originY: leaf.top + grabY * leaf.height - axisY * leaf.width * progress,
    axisX,
    axisY,
    radius: 0.16 * leaf.width * (1 - progress * progress),
    progress,
  };
}
