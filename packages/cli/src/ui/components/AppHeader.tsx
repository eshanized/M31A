/**
 * @license
 * Copyright 2026 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import { Box, Text } from 'ink';
import { UserIdentity } from './UserIdentity.js';
import { Tips } from './Tips.js';
import { useSettings } from '../contexts/SettingsContext.js';
import { useConfig } from '../contexts/ConfigContext.js';
import { useUIState } from '../contexts/UIStateContext.js';
import { Banner } from './Banner.js';
import { useBanner } from '../hooks/useBanner.js';
import { useTips } from '../hooks/useTips.js';
import { theme } from '../semantic-colors.js';
import { ThemedGradient } from './ThemedGradient.js';
import { CliSpinner } from './CliSpinner.js';

import { getMascotIcon, longAsciiLogoCompactText } from './AsciiArt.js';
import { getAsciiArtWidth } from '../utils/textUtils.js';
import { isCompactWidth } from '../utils/isNarrowWidth.js';

interface AppHeaderProps {
  version: string;
  showDetails?: boolean;
}

/**
 * The horizontal padding (in columns) required for metadata (version, identity, etc.)
 * when rendered alongside the ASCII logo.
 */
const LOGO_METADATA_PADDING = 20;

export const AppHeader = ({ version, showDetails = true }: AppHeaderProps) => {
  const settings = useSettings();
  const config = useConfig();
  const {
    terminalWidth,
    mainAreaWidth,
    bannerData,
    bannerVisible,
    updateInfo,
    isConfigInitialized,
  } = useUIState();
  const contentWidth = mainAreaWidth || terminalWidth;

  const { bannerText } = useBanner(bannerData);
  const { showTips } = useTips();

  const authType = config.getContentGeneratorConfig()?.authType;
  const loggedOut = isConfigInitialized && !authType;

  const showHeader = !(
    settings.merged.ui.hideBanner || config.getScreenReader()
  );

  const ICON = getMascotIcon();

  let logoTextArt = '';
  if (loggedOut) {
    const widthOfLongLogo =
      getAsciiArtWidth(longAsciiLogoCompactText) + LOGO_METADATA_PADDING;

    if (contentWidth >= widthOfLongLogo) {
      logoTextArt = longAsciiLogoCompactText.trim();
    }
  }

  // If the pane is too narrow to fit the icon and metadata side-by-side,
  // switch to column mode to prevent wrapping.
  const isNarrow = isCompactWidth(contentWidth);

  const renderLogo = () => (
    <Box flexDirection="row">
      <Box flexShrink={0}>
        <ThemedGradient>{ICON}</ThemedGradient>
      </Box>
      {logoTextArt && (
        <Box marginLeft={3}>
          <Text color={theme.text.primary}>{logoTextArt}</Text>
        </Box>
      )}
    </Box>
  );

  const renderMetadata = (isBelow = false) => (
    <Box marginLeft={isBelow ? 0 : 2} flexDirection="column">
      {/* Line 1: M31A vVersion [Updating] */}
      <Box>
        <Text bold color={theme.text.accent}>
          ◆ M31A
        </Text>
        <Text color={theme.text.secondary}> v{version}</Text>
        {updateInfo?.isUpdating && (
          <Box marginLeft={2}>
            <Text color={theme.text.secondary}>
              <CliSpinner altText="Updating" /> Updating
            </Text>
          </Box>
        )}
      </Box>

      {showDetails && (
        <>
          <Box height={1} />
          {settings.merged.ui.showUserIdentity !== false && (
            <UserIdentity config={config} />
          )}
        </>
      )}
    </Box>
  );

  const useColumnLayout = !!logoTextArt || isNarrow;

  return (
    <Box flexDirection="column" width="100%">
      {showHeader && (
        <Box
          flexDirection={useColumnLayout ? 'column' : 'row'}
          marginTop={1}
          marginBottom={1}
          paddingX={1}
          borderStyle="round"
          borderColor={theme.border.default}
        >
          {renderLogo()}
          {useColumnLayout ? (
            <Box marginTop={1}>{renderMetadata(true)}</Box>
          ) : (
            renderMetadata(false)
          )}
        </Box>
      )}

      {bannerVisible && bannerText && (
        <Banner
          width={contentWidth}
          bannerText={bannerText}
          isWarning={bannerData.warningText !== ''}
        />
      )}

      {!(settings.merged.ui.hideTips || config.getScreenReader()) &&
        showTips && <Tips config={config} />}
    </Box>
  );
};
