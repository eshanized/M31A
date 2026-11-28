/**
 * @license
 * Copyright 2026 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

interface M31aLogoProps {
  size?: number;
  idPrefix?: string;
}

/**
 * Official M31A SVG emblem matching repository root M31A.svg.
 */
export function M31aLogo({ size = 24, idPrefix = 'm31a' }: M31aLogoProps) {
  const leftLegId = `${idPrefix}-left-leg`;
  const rightLegId = `${idPrefix}-right-leg`;
  const topFaceId = `${idPrefix}-top-face`;
  const leftFacetId = `${idPrefix}-left-facet`;
  const rightFacetId = `${idPrefix}-right-facet`;
  const glowId = `${idPrefix}-soft-glow`;

  return (
    <svg
      xmlns="http://www.w3.org/2000/svg"
      viewBox="0 0 512 512"
      width={size}
      height={size}
      role="img"
      aria-label="M31A logo"
    >
      <defs>
        <linearGradient id={leftLegId} x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stopColor="#9a68ff" />
          <stop offset="0.52" stopColor="#8650ed" />
          <stop offset="1" stopColor="#6636ca" />
        </linearGradient>
        <linearGradient id={rightLegId} x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stopColor="#a66cff" />
          <stop offset="0.48" stopColor="#8750ec" />
          <stop offset="1" stopColor="#6738c9" />
        </linearGradient>
        <linearGradient id={topFaceId} x1="0" y1="0" x2="1" y2="0.8">
          <stop offset="0" stopColor="#a978ff" />
          <stop offset="0.48" stopColor="#bd85ff" />
          <stop offset="1" stopColor="#a76bfa" />
        </linearGradient>
        <linearGradient id={leftFacetId} x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stopColor="#9a69f5" stopOpacity="0.05" />
          <stop offset="1" stopColor="#6537cb" stopOpacity="0.42" />
        </linearGradient>
        <linearGradient id={rightFacetId} x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stopColor="#d09aff" stopOpacity="0.15" />
          <stop offset="1" stopColor="#6637ce" stopOpacity="0.34" />
        </linearGradient>
        <filter id={glowId} x="-20%" y="-20%" width="140%" height="140%">
          <feGaussianBlur stdDeviation="5" result="blur" />
          <feColorMatrix
            in="blur"
            type="matrix"
            values="0.55 0 0 0 0.30  0 0.20 0 0 0.08  0 0 0.95 0 0.55  0 0 0 0.42 0"
            result="glow"
          />
          <feMerge>
            <feMergeNode in="glow" />
            <feMergeNode in="SourceGraphic" />
          </feMerge>
        </filter>
      </defs>

      <g filter={`url(#${glowId})`}>
        <path
          d="M34 132 L151 225 V423 Q151 439 138 447 L49 395 Q34 386 34 369 Z"
          fill={`url(#${leftLegId})`}
        />
        <path
          d="M361 225 L478 132 V369 Q478 386 463 395 L374 447 Q361 439 361 423 Z"
          fill={`url(#${rightLegId})`}
        />
        <path
          d="M34 132 L151 225 V423 Q151 439 138 447 L49 395 Q34 386 34 369 Z"
          fill={`url(#${leftFacetId})`}
        />
        <path
          d="M361 225 L478 132 V369 Q478 386 463 395 L374 447 Q361 439 361 423 Z"
          fill={`url(#${rightFacetId})`}
        />
        <path
          d="M38 111 L111 47 Q121 38 132 47 L256 151 L380 47 Q391 38 402 47 L474 109 Q483 117 473 126 L268 291 Q256 301 244 291 L39 126 Q29 117 38 111 Z"
          fill={`url(#${topFaceId})`}
        />
        <path
          d="M38 111 L111 47 Q121 38 132 47 L256 151 L174 223 L39 126 Q29 117 38 111 Z"
          fill="#9b6cf4"
          opacity="0.18"
        />
        <path
          d="M256 151 L380 47 Q391 38 402 47 L474 109 Q483 117 473 126 L338 235 Z"
          fill="#d5a0ff"
          opacity="0.16"
        />
        <path
          d="M174 223 L256 151 L338 235 L268 291 Q256 301 244 291 Z"
          fill="#c68aff"
          opacity="0.20"
        />
      </g>
    </svg>
  );
}
