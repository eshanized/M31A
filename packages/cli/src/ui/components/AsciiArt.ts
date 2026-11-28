/**
 * @license
 * Copyright 2025 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import { isAppleTerminal } from '@m31a/core';

/**
 * Quadrant-block mark rasterized directly from M31A.svg.
 */
export const DEFAULT_MASCOT_ICON = `▄█▙▖▗▟█▄
████████
██ ▀▀ ██
▀█    █▀`;

/**
 * Symmetric half-block variant of M31A.svg for Apple Terminal.app line-height spacing.
 */
export const MAC_TERMINAL_MASCOT_ICON = `▄█▄  ▄█▄
████████
██ ▀▀ ██
▀█    █▀`;

export function getMascotIcon(): string {
  return isAppleTerminal() ? MAC_TERMINAL_MASCOT_ICON : DEFAULT_MASCOT_ICON;
}

export const shortAsciiLogo = `
 ▄█▙▖    ▗▟█▄
▟████▙▖▗▟█████
██████████████
████▀▜██▛▀████
████  ▝▘  ████
████      ████
 ▀▜▛      ▜▛▀
`;

export const longAsciiLogo = `
  ▗▄█▄▖        ▗▄█▙▖
▗▟█████▙▖    ▗▟█████▙▖
██████████▄▄██████████
██████████████████████
██████▀████████▀██████
██████  ▀▜██▛▀  ██████
██████    ▝▘    ██████
██████          ██████
▀█████          █████▀
   ▀▜▛          ▜▛▀
`;

export const tinyAsciiLogo = `
▄█▙▖▗▟█▄
████████
██ ▀▀ ██
▀█    █▀
`;

export const shortAsciiLogoCompactText = `
▟█▙▄▟█▙ ▀▀▜▖▗▜ ▗▟▀▙
██▀█▀██ ▀▄█▌ ▐ ████
▜█   █▛ ▄▄█▌▄▟▄█▌▐█
`;

export const longAsciiLogoCompactText = `
▟██▖▗██▙ ▗▀▀▀▜▖ ▗█▙  ▗▟█▀█▙ 
█▌▝█▟▘▐█    ▄█▘  ▐█  █▌  ▐█ 
█▌ ▝▘ ▐█ ▗  ▝█▖  ▐█  ██████ 
▀▘    ▝▀ ▝▀▀▀▘  ▝▀▀▘ ▀▘  ▝▀ 
`;

export const tinyAsciiLogoCompactText = `
▄█▙▖▗▟█▄
████████
██ ▀▀ ██
▀█    █▀
`;
