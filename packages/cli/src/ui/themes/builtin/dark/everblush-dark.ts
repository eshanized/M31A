/**
 * @license
 * Copyright 2026 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import { type ColorsTheme, Theme } from '../../theme.js';

// Canonical Everblush dark palette
const palette = {
  bg: '#141b1e',
  bgAlt: '#1e2528',
  bgHighlight: '#232a2d',
  bgGutter: '#2d3437',
  fg: '#dadada',
  fgMuted: '#b3b9b8',
  comment: '#6c7477',
  red: '#e57474',
  green: '#8ccf7e',
  yellow: '#e5c76b',
  blue: '#67b0e8',
  magenta: '#c47fd5',
  cyan: '#6cbfbf',
  diffAdd: '#1d3324',
  diffDelete: '#382023',
};

const everblushColors: ColorsTheme = {
  type: 'dark',
  Background: palette.bg,
  Foreground: palette.fg,
  LightBlue: palette.cyan,
  AccentBlue: palette.blue,
  AccentPurple: palette.magenta,
  AccentCyan: palette.cyan,
  AccentGreen: palette.green,
  AccentYellow: palette.yellow,
  AccentRed: palette.red,
  DiffAdded: palette.diffAdd,
  DiffRemoved: palette.diffDelete,
  Comment: palette.comment,
  Gray: palette.fgMuted,
  DarkGray: palette.bgGutter,
  InputBackground: palette.bgAlt,
  MessageBackground: palette.bgHighlight,
  FocusBackground: palette.bgHighlight,
  FocusColor: palette.green,
  GradientColors: [palette.green, palette.cyan, palette.blue],
};

export const Everblush: Theme = new Theme(
  'Everblush',
  'dark',
  {
    hljs: {
      display: 'block',
      overflowX: 'auto',
      padding: '0.5em',
      background: palette.bg,
      color: palette.fg,
    },
    'hljs-keyword': {
      color: palette.magenta,
    },
    'hljs-literal': {
      color: palette.yellow,
    },
    'hljs-symbol': {
      color: palette.cyan,
    },
    'hljs-name': {
      color: palette.red,
    },
    'hljs-link': {
      color: palette.blue,
      textDecoration: 'underline',
    },
    'hljs-built_in': {
      color: palette.cyan,
    },
    'hljs-type': {
      color: palette.yellow,
    },
    'hljs-number': {
      color: palette.yellow,
    },
    'hljs-class': {
      color: palette.yellow,
    },
    'hljs-string': {
      color: palette.green,
    },
    'hljs-meta-string': {
      color: palette.green,
    },
    'hljs-regexp': {
      color: palette.cyan,
    },
    'hljs-template-tag': {
      color: palette.magenta,
    },
    'hljs-subst': {
      color: palette.fg,
    },
    'hljs-function': {
      color: palette.blue,
    },
    'hljs-title': {
      color: palette.blue,
    },
    'hljs-params': {
      color: palette.fg,
    },
    'hljs-formula': {
      color: palette.fg,
    },
    'hljs-comment': {
      color: palette.comment,
      fontStyle: 'italic',
    },
    'hljs-quote': {
      color: palette.comment,
      fontStyle: 'italic',
    },
    'hljs-doctag': {
      color: palette.cyan,
    },
    'hljs-meta': {
      color: palette.fgMuted,
    },
    'hljs-meta-keyword': {
      color: palette.magenta,
    },
    'hljs-tag': {
      color: palette.red,
    },
    'hljs-variable': {
      color: palette.fg,
    },
    'hljs-template-variable': {
      color: palette.red,
    },
    'hljs-attr': {
      color: palette.yellow,
    },
    'hljs-attribute': {
      color: palette.yellow,
    },
    'hljs-builtin-name': {
      color: palette.cyan,
    },
    'hljs-section': {
      color: palette.blue,
      fontWeight: 'bold',
    },
    'hljs-emphasis': {
      fontStyle: 'italic',
    },
    'hljs-strong': {
      fontWeight: 'bold',
    },
    'hljs-bullet': {
      color: palette.yellow,
    },
    'hljs-selector-tag': {
      color: palette.red,
    },
    'hljs-selector-id': {
      color: palette.blue,
    },
    'hljs-selector-class': {
      color: palette.yellow,
    },
    'hljs-selector-attr': {
      color: palette.cyan,
    },
    'hljs-selector-pseudo': {
      color: palette.magenta,
    },
    'hljs-addition': {
      backgroundColor: palette.diffAdd,
      display: 'inline-block',
      width: '100%',
    },
    'hljs-deletion': {
      backgroundColor: palette.diffDelete,
      display: 'inline-block',
      width: '100%',
    },
  },
  everblushColors,
);
