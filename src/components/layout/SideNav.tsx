import { containsWithScrollbar } from '../../utils/scrollbarContainment';
import { Settings, Rocket, GaugeCircle, LayoutGrid, SlidersHorizontal, FileText, ChevronDown, PanelLeftClose, PanelLeftOpen, ShieldCheck } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useState, useRef, useCallback, useEffect, useLayoutEffect, useMemo, type CSSProperties } from 'react';
import { createPortal } from 'react-dom';
import apiKeyFunIcon from '../../assets/icons/apikey-fun.png';
import { Page } from '../../types/navigation';
import { isMenuVisiblePlatform, PlatformId, PLATFORM_PAGE_MAP } from '../../types/platform';
import {
  API_RELAY_LAYOUT_ENTRY_ID,
  ApiRelayLayoutEntryId,
  makePlatformEntryId,
  resolveGroupChildIcon,
  resolveGroupChildName,
  parseGroupEntryId,
  parsePlatformEntryId,
  PlatformLayoutEntryId,
  PlatformLayoutGroup,
  resolveEntryDefaultPlatformId,
  resolveEntryIdForPlatform,
  usePlatformLayoutStore,
} from '../../stores/usePlatformLayoutStore';
import { useSideNavLayoutStore } from '../../stores/useSideNavLayoutStore';
import { useGlobalModal } from '../../hooks/useGlobalModal';
import { getPlatformLabel, renderPlatformIcon } from '../../utils/platformMeta';
import { useRemoteConfigStore } from '../../stores/useRemoteConfigStore';
import { useApiKeyFunCampaign } from '../../hooks/useApiKeyFunCampaign';
import './SideNavCampaign.css';

interface SideNavProps {
  page: Page;
  setPage: (page: Page) => void;
  onOpenPlatformLayout: () => void;
  easterEggClickCount: number;
  onEasterEggTriggerClick: () => void;
  hasBreakoutSession: boolean;
  updateActionState: 'hidden' | 'available' | 'downloading' | 'installing' | 'ready';
  updateProgress: number;
  onUpdateActionClick: () => void;
  updateRemindersEnabled: boolean;
  sponsorEntryVisible: boolean;
  onOpenLogViewer: () => void;
}

interface FlyingRocket {
  id: number;
  x: number;
}

type SideNavEntryId = PlatformLayoutEntryId | ApiRelayLayoutEntryId;

interface SideNavEntry {
  id: SideNavEntryId;
  kind: 'platform' | 'api-relay';
  label: string;
  hidden: boolean;
  targetPlatformId: PlatformId | null;
  platformIds: PlatformId[];
  group: PlatformLayoutGroup | null;
  childIcon?: ReturnType<typeof resolveGroupChildIcon>;
}

const PAGE_PLATFORM_MAP: Partial<Record<Page, PlatformId>> = {
  overview: 'antigravity',
  codex: 'codex',
  'codex-api-service': 'codex_api_service',
  claude: 'claude_manager',
  'claude-cli': 'claude_manager',
  zed: 'zed',
  'github-copilot': 'github-copilot',
  windsurf: 'windsurf',
  kiro: 'kiro',
  cursor: 'cursor',
  grok: 'grok',
  codebuddy: 'codebuddy',
  'codebuddy-cn': 'codebuddy_cn',
  qoder: 'qoder',
  'qoder-cn': 'qoder_cn',
  zcode: 'zcode',
  t3code: 't3code',
  trae: 'trae',
  'trae-solo': 'trae_solo',
  'trae-cn': 'trae_cn',
  'trae-solo-cn': 'trae_solo_cn',
  workbuddy: 'workbuddy',
  workbuddy_ai: 'workbuddy_ai',
  minimaxcode: 'minimaxcode',
};

const APP_DISPLAY_NAME =
  import.meta.env.VITE_COCKPIT_TOOLS_PROFILE === 'dev' ? 'Cockpit Tools Dev' : 'Cockpit Tools';

function renderEntryIcon(entry: SideNavEntry, size: number) {
  if (entry.kind === 'api-relay') {
    return (
      <img
        className="nav-item-icon"
        src={apiKeyFunIcon}
        alt=""
        width={size}
        height={size}
      />
    );
  }

  if (entry.childIcon) {
    const { iconKind, iconCustomDataUrl, iconPlatformId } = entry.childIcon;
    return iconKind === 'custom' && iconCustomDataUrl ? (
      <img
        src={iconCustomDataUrl}
        alt=""
        className="side-nav-group-icon"
        width={size}
        height={size}
      />
    ) : renderPlatformIcon(iconPlatformId, size);
  }

  if (entry.group && entry.group.iconKind === 'custom' && entry.group.iconCustomDataUrl) {
    return (
      <img
        src={entry.group.iconCustomDataUrl}
        alt={entry.label}
        className="side-nav-group-icon"
        style={{ width: size, height: size }}
      />
    );
  }

  if (entry.group) {
    const iconPlatform = entry.group.iconPlatformId ?? entry.targetPlatformId;
    return iconPlatform ? renderPlatformIcon(iconPlatform, size) : null;
  }

  return entry.targetPlatformId ? renderPlatformIcon(entry.targetPlatformId, size) : null;
}

function isAntigravitySuitePage(page: Page): boolean {
  return (
    page === 'overview' ||
    page === 'antigravity-usage' ||
    page === 'instances' ||
    page === 'wakeup' ||
    page === 'verification'
  );
}

export function SideNav({
  page,
  setPage,
  onOpenPlatformLayout,
  easterEggClickCount,
  onEasterEggTriggerClick,
  hasBreakoutSession,
  updateActionState,
  updateProgress,
  onUpdateActionClick,
  updateRemindersEnabled,
  sponsorEntryVisible,
  onOpenLogViewer,
}: SideNavProps) {
  const { t } = useTranslation();
  const campaignActive = useApiKeyFunCampaign();
  const { showModal } = useGlobalModal();
  const [flyingRockets, setFlyingRockets] = useState<FlyingRocket[]>([]);
  const [showMore, setShowMore] = useState(false);
  const [classicHandleTop, setClassicHandleTop] = useState<number | null>(null);
  const [morePopoverPosition, setMorePopoverPosition] = useState({
    top: 120,
    left: 210,
    maxHeight: 560,
  });
  const sideNavLayoutMode = useSideNavLayoutStore((state) => state.mode);
  const setSideNavLayoutMode = useSideNavLayoutStore((state) => state.setMode);
  const classicCollapsed = useSideNavLayoutStore((state) => state.classicCollapsed);
  const toggleClassicCollapsed = useSideNavLayoutStore((state) => state.toggleClassicCollapsed);
  const hideClassicSwitchPrompt = useSideNavLayoutStore((state) => state.hideClassicSwitchPrompt);
  const setHideClassicSwitchPrompt = useSideNavLayoutStore((state) => state.setHideClassicSwitchPrompt);
  const isClassicLayout = sideNavLayoutMode === 'classic';
  const isClassicCollapsed = isClassicLayout && classicCollapsed;
  const showClassicLabels = isClassicLayout && !classicCollapsed;
  const rocketIdRef = useRef(0);
  const classicSwitchDontAskAgainRef = useRef(false);
  const logoRef = useRef<HTMLDivElement>(null);
  const morePopoverRef = useRef<HTMLDivElement>(null);
  const moreButtonRef = useRef<HTMLButtonElement>(null);

  const {
    orderedEntryIds,
    hiddenEntryIds,
    sidebarEntryIds,
    platformGroups,
    apiRelaySidebarVisible,
    apiRelayEntryOrder,
  } = usePlatformLayoutStore();
  const remoteHiddenPlatformIds = useRemoteConfigStore((state) => state.hiddenPlatformIds);

  const currentPlatformId = isAntigravitySuitePage(page)
    ? 'antigravity'
    : PAGE_PLATFORM_MAP[page] ?? null;
  const hiddenSet = useMemo(() => new Set(hiddenEntryIds), [hiddenEntryIds]);
  const sidebarSet = useMemo(() => new Set(sidebarEntryIds), [sidebarEntryIds]);
  const remoteHiddenPlatformSet = useMemo(
    () => new Set(remoteHiddenPlatformIds),
    [remoteHiddenPlatformIds],
  );
  const isPlatformAvailable = useCallback(
    (platformId: PlatformId) =>
      isMenuVisiblePlatform(platformId) && !remoteHiddenPlatformSet.has(platformId),
    [remoteHiddenPlatformSet],
  );
  const apiRelayEntryVisible = sponsorEntryVisible && apiRelaySidebarVisible;

  const orderedEntries = useMemo<SideNavEntry[]>(() => {
    const platformEntries: SideNavEntry[] = orderedEntryIds
      .map<SideNavEntry | null>((entryId) => {
        const platformId = parsePlatformEntryId(entryId);
        if (platformId) {
          if (!isPlatformAvailable(platformId)) {
            return null;
          }
          return {
            id: entryId,
            kind: 'platform' as const,
            label: getPlatformLabel(platformId, t),
            hidden: hiddenSet.has(entryId),
            targetPlatformId: platformId,
            platformIds: [platformId],
            group: null,
          };
        }

        const groupId = parseGroupEntryId(entryId);
        if (!groupId) {
          return null;
        }
        const group = platformGroups.find((item) => item.id === groupId);
        if (!group) {
          return null;
        }

        const visiblePlatformIds = group.platformIds.filter(isPlatformAvailable);
        if (visiblePlatformIds.length === 0) {
          return null;
        }

        const resolvedTargetPlatformId = resolveEntryDefaultPlatformId(entryId, platformGroups);
        const targetPlatformId = resolvedTargetPlatformId && visiblePlatformIds.includes(resolvedTargetPlatformId)
          ? resolvedTargetPlatformId
          : visiblePlatformIds[0];
        if (!targetPlatformId) {
          return null;
        }

        return {
          id: entryId,
          kind: 'platform' as const,
          label: group.name,
          hidden: hiddenSet.has(entryId),
          targetPlatformId,
          platformIds: visiblePlatformIds,
          group,
        };
      })
      .filter((entry): entry is SideNavEntry => !!entry);

    if (!apiRelayEntryVisible) {
      return platformEntries;
    }

    const result = [...platformEntries];
    const insertIndex = Math.max(0, Math.min(apiRelayEntryOrder, result.length));
    result.splice(insertIndex, 0, {
      id: API_RELAY_LAYOUT_ENTRY_ID,
      kind: 'api-relay',
      label: t('nav.apiRelay', '中转站'),
      hidden: false,
      targetPlatformId: null,
      platformIds: [],
      group: null,
    });
    return result;
  }, [
    apiRelayEntryOrder,
    apiRelayEntryVisible,
    orderedEntryIds,
    platformGroups,
    hiddenSet,
    isPlatformAvailable,
    t,
  ]);

  const sidebarVisibleEntries = useMemo(() => orderedEntries.flatMap((entry) => {
    const visible = entry.kind === 'api-relay' || sidebarSet.has(entry.id as PlatformLayoutEntryId)
      ? [entry]
      : [];
    const group = entry.group;
    if (!group) return visible;

    for (const platformId of entry.platformIds) {
      const id = makePlatformEntryId(platformId);
      if (!sidebarSet.has(id)) continue;
      visible.push({
        id,
        kind: 'platform',
        label: resolveGroupChildName(group, platformId, getPlatformLabel(platformId, t)),
        hidden: entry.hidden,
        targetPlatformId: platformId,
        platformIds: [platformId],
        group: null,
        childIcon: resolveGroupChildIcon(group, platformId),
      });
    }
    return visible;
  }), [orderedEntries, sidebarSet, t]);

  const sidebarMenuEntries = sidebarVisibleEntries;

  const navigateToPlatform = useCallback((platformId: PlatformId) => {
    setPage(PLATFORM_PAGE_MAP[platformId]);
  }, [setPage]);

  const navigateToEntry = useCallback((entry: SideNavEntry) => {
    if (entry.kind === 'api-relay') {
      setPage('api-relay');
      return;
    }
    if (entry.targetPlatformId) {
      navigateToPlatform(entry.targetPlatformId);
    }
  }, [navigateToPlatform, setPage]);

  const sidebarMenuEntryIdSet = useMemo(
    () => new Set(sidebarMenuEntries.map((entry) => entry.id)),
    [sidebarMenuEntries],
  );
  let currentEntryId: SideNavEntryId | null = null;
  if (page === 'api-relay') {
    currentEntryId = API_RELAY_LAYOUT_ENTRY_ID;
  } else if (currentPlatformId) {
    const individualEntryId = makePlatformEntryId(currentPlatformId);
    currentEntryId = sidebarMenuEntryIdSet.has(individualEntryId)
      ? individualEntryId
      : resolveEntryIdForPlatform(currentPlatformId, platformGroups);
  }

  const sidebarMenuPlatformIdSet = useMemo(
    () => new Set(
      sidebarMenuEntries
        .map((entry) => entry.targetPlatformId)
        .filter((platformId): platformId is PlatformId => !!platformId),
    ),
    [sidebarMenuEntries],
  );

  const moreMenuEntries = useMemo<SideNavEntry[]>(
    () => {
      if (isClassicLayout) {
        return orderedEntries.filter((entry) => !sidebarMenuEntryIdSet.has(entry.id));
      }

      return orderedEntries
        .map((entry) => {
          if (entry.kind === 'api-relay') {
            return sidebarMenuEntryIdSet.has(entry.id) ? null : entry;
          }
          const remainingPlatformIds = entry.platformIds.filter(
            (platformId) => !sidebarMenuPlatformIdSet.has(platformId),
          );
          if (remainingPlatformIds.length === 0) {
            return null;
          }
          const resolvedTargetPlatformId = entry.targetPlatformId && remainingPlatformIds.includes(entry.targetPlatformId)
            ? entry.targetPlatformId
            : remainingPlatformIds[0];
          return {
            ...entry,
            targetPlatformId: resolvedTargetPlatformId,
            platformIds: remainingPlatformIds,
          };
        })
        .filter((entry): entry is SideNavEntry => !!entry);
    },
    [isClassicLayout, orderedEntries, sidebarMenuEntryIdSet, sidebarMenuPlatformIdSet],
  );

  const isMoreActive = !!currentEntryId && !sidebarMenuEntryIdSet.has(currentEntryId);
  const shouldLockActiveOnMore = showMore;

  const shouldShowUpdateActionEntry = updateActionState !== 'hidden'
    && (
      updateRemindersEnabled
      || updateActionState === 'downloading'
      || updateActionState === 'installing'
      || updateActionState === 'ready'
    );
  const classicHandleStyle = classicHandleTop == null ? undefined : { top: `${classicHandleTop}px` };

  const handleClassicLayoutEntryClick = useCallback(() => {
    if (hideClassicSwitchPrompt) {
      setSideNavLayoutMode('classic');
      return;
    }
    classicSwitchDontAskAgainRef.current = false;
    showModal({
      title: t('nav.switchClassicLayoutTitle', '切换至经典侧边栏布局'),
      description: t(
        'nav.switchClassicLayoutDesc',
        '经典布局会展示完整平台导航并支持折叠。你仍可在“设置 > 通用 > 侧边栏布局”中随时切换回原始布局。',
      ),
      width: 'sm',
      content: (
        <div className="side-nav-layout-switch-modal-content">
          <label className="side-nav-layout-switch-remember">
            <input
              type="checkbox"
              onChange={(event) => {
                classicSwitchDontAskAgainRef.current = event.target.checked;
              }}
            />
            <span>{t('nav.switchClassicLayoutDontAskAgain', '不再提示此引导')}</span>
          </label>
        </div>
      ),
      actions: [
        {
          id: 'classic-switch-cancel',
          label: t('common.cancel', '取消'),
          variant: 'secondary',
        },
        {
          id: 'classic-switch-confirm',
          label: t('nav.switchClassicLayoutConfirm', '立即切换'),
          variant: 'primary',
          onClick: () => {
            if (classicSwitchDontAskAgainRef.current) {
              setHideClassicSwitchPrompt(true);
            }
            setSideNavLayoutMode('classic');
          },
        },
      ],
    });
  }, [hideClassicSwitchPrompt, setHideClassicSwitchPrompt, setSideNavLayoutMode, showModal, t]);

  useLayoutEffect(() => {
    if (!isClassicLayout || typeof window === 'undefined') {
      setClassicHandleTop(null);
      return;
    }

    const updateClassicHandleTop = () => {
      const rect = logoRef.current?.getBoundingClientRect();
      if (!rect) return;
      setClassicHandleTop(rect.top + rect.height / 2);
    };

    updateClassicHandleTop();
    const rafId = window.requestAnimationFrame(updateClassicHandleTop);
    const resizeObserver = typeof ResizeObserver !== 'undefined' && logoRef.current
      ? new ResizeObserver(() => {
        updateClassicHandleTop();
      })
      : null;

    if (resizeObserver && logoRef.current) {
      resizeObserver.observe(logoRef.current);
    }

    window.addEventListener('resize', updateClassicHandleTop);

    return () => {
      window.cancelAnimationFrame(rafId);
      window.removeEventListener('resize', updateClassicHandleTop);
      resizeObserver?.disconnect();
    };
  }, [isClassicLayout, isClassicCollapsed, shouldShowUpdateActionEntry]);

  const handleLogoClick = useCallback(() => {
    if (hasBreakoutSession) {
      onEasterEggTriggerClick();
      return;
    }

    const newRocket: FlyingRocket = {
      id: rocketIdRef.current++,
      x: (Math.random() - 0.5) * 40,
    };

    setFlyingRockets((prev) => [...prev, newRocket]);

    setTimeout(() => {
      setFlyingRockets((prev) => prev.filter((rocket) => rocket.id !== newRocket.id));
    }, 1500);

    onEasterEggTriggerClick();
  }, [hasBreakoutSession, onEasterEggTriggerClick]);

  useEffect(() => {
    if (!showMore) return;
    const handleClickOutside = (event: MouseEvent) => {
      const target = event.target as Node;
      if (containsWithScrollbar(morePopoverRef.current, target)) return;
      if (containsWithScrollbar(moreButtonRef.current, target)) return;
      setShowMore(false);
    };
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, [showMore]);

  useEffect(() => {
    if (!showMore) return;

    const updatePopoverPosition = () => {
      const button = moreButtonRef.current;
      const nav = button?.closest('.side-nav');
      const popover = morePopoverRef.current;
      if (!button || !nav || !popover) return;

      const navRect = nav.getBoundingClientRect();
      const buttonRect = button.getBoundingClientRect();
      const safeTop = 52;
      const safeBottom = 16;
      const popoverHeight = Math.max(180, popover.offsetHeight);
      const preferredTop = buttonRect.top + buttonRect.height / 2 - popoverHeight / 2;
      const maxTop = Math.max(safeTop, window.innerHeight - safeBottom - popoverHeight);
      const top = Math.max(safeTop, Math.min(preferredTop, maxTop));
      const maxHeight = Math.max(180, window.innerHeight - top - safeBottom);

      setMorePopoverPosition({
        top,
        left: navRect.right - 1,
        maxHeight,
      });
    };

    updatePopoverPosition();
    const rafId = window.requestAnimationFrame(updatePopoverPosition);
    window.addEventListener('resize', updatePopoverPosition);
    window.addEventListener('scroll', updatePopoverPosition, true);
    const resizeObserver = typeof ResizeObserver !== 'undefined' && morePopoverRef.current
      ? new ResizeObserver(() => {
        updatePopoverPosition();
      })
      : null;
    if (resizeObserver && morePopoverRef.current) {
      resizeObserver.observe(morePopoverRef.current);
    }

    return () => {
      window.cancelAnimationFrame(rafId);
      window.removeEventListener('resize', updatePopoverPosition);
      window.removeEventListener('scroll', updatePopoverPosition, true);
      resizeObserver?.disconnect();
    };
  }, [showMore, isClassicLayout]);

  const clampedUpdateProgress = Math.max(0, Math.min(100, Math.round(updateProgress)));
  const updateVisualState = updateActionState === 'ready'
    ? 'restart'
    : updateActionState === 'downloading' || updateActionState === 'installing'
      ? 'progress'
      : 'update';
  const updateEntryTitle = updateActionState === 'downloading'
    ? t('update_notification.downloading', '下载中...')
    : updateActionState === 'installing'
      ? t('nav.quickUpdate.installing', '安装中')
      : updateActionState === 'ready'
        ? t('nav.quickUpdate.restart', '重启')
        : t('nav.quickUpdate.update', '更新');
  const updateEntryText = updateActionState === 'ready'
    ? t('nav.quickUpdate.restart', '重启')
    : t('nav.quickUpdate.update', '更新');

  const morePopoverContent = showMore ? (
    <div
      className={`side-nav-more-popover${isClassicLayout ? ' side-nav-more-popover-classic' : ''}`}
      ref={morePopoverRef}
      style={{ position: 'fixed', top: `${morePopoverPosition.top}px`, left: `${morePopoverPosition.left}px`, maxHeight: `${morePopoverPosition.maxHeight}px`, transform: 'none' }}
    >
      <div className="side-nav-more-title">{t('nav.morePlatforms', '更多平台')}</div>
      <div className="side-nav-more-list">
        {moreMenuEntries.map((entry) => {
          const active = entry.kind === 'api-relay'
            ? currentEntryId === entry.id
            : isClassicLayout
              ? currentEntryId === entry.id
              : !!currentPlatformId && entry.platformIds.includes(currentPlatformId);
          const showGroupParent =
            !entry.group || !sidebarMenuEntryIdSet.has(entry.id);
          return (
            <div className="side-nav-more-group" key={entry.id}>
              {(isClassicLayout || showGroupParent) && (
                <button
                  className={`side-nav-more-item ${active ? 'active' : ''}`}
                  onClick={() => {
                    navigateToEntry(entry);
                    setShowMore(false);
                  }}
                >
                  <span className="side-nav-more-item-icon">{renderEntryIcon(entry, 16)}</span>
                  <span className="side-nav-more-item-label">{entry.label}</span>
                  {entry.kind === 'api-relay' && campaignActive && (
                    <span className="api-relay-campaign-badge">{t('apiKeyFun.campaign.badge')}</span>
                  )}
                  {entry.hidden && (
                    <span className="side-nav-more-item-badge">
                      {t('platformLayout.hiddenBadge', '已隐藏')}
                    </span>
                  )}
                </button>
              )}

              {!isClassicLayout && entry.group && entry.platformIds.length > (showGroupParent ? 1 : 0) && (
                <div className={`side-nav-more-sub-list${showGroupParent ? '' : ' is-flat'}`}>
                  {entry.platformIds.map((platformId) => {
                    const icon = resolveGroupChildIcon(entry.group!, platformId);
                    const label = resolveGroupChildName(
                      entry.group!,
                      platformId,
                      getPlatformLabel(platformId, t),
                    );
                    return (
                      <button
                        key={`${entry.id}:${platformId}`}
                        className={`${
                          showGroupParent ? 'side-nav-more-sub-item' : 'side-nav-more-item'
                        } ${currentPlatformId === platformId ? 'active' : ''}`}
                        onClick={() => {
                          navigateToPlatform(platformId);
                          setShowMore(false);
                        }}
                      >
                        <span className={showGroupParent ? 'side-nav-more-sub-item-icon' : 'side-nav-more-item-icon'}>
                          {icon.iconKind === 'custom' && icon.iconCustomDataUrl ? (
                            <img
                              src={icon.iconCustomDataUrl}
                              alt={label}
                              className="side-nav-group-icon"
                              style={{ width: showGroupParent ? 14 : 16, height: showGroupParent ? 14 : 16 }}
                            />
                          ) : (
                            renderPlatformIcon(icon.iconPlatformId, showGroupParent ? 14 : 16)
                          )}
                        </span>
                        <span className={showGroupParent ? 'side-nav-more-sub-item-label' : 'side-nav-more-item-label'}>
                          {label}
                        </span>
                      </button>
                    );
                  })}
                </div>
              )}
            </div>
          );
        })}
      </div>
      <button
        className="side-nav-more-manage"
        onClick={() => {
          setShowMore(false);
          onOpenPlatformLayout();
        }}
      >
        <SlidersHorizontal size={14} />
        <span>{t('platformLayout.openFromMore', '管理平台布局')}</span>
      </button>
    </div>
  ) : null;

  return (
    <>
      <nav
        className={`side-nav${isClassicLayout ? ' side-nav-classic' : ''}${isClassicCollapsed ? ' side-nav-classic-collapsed' : ''}`}
      >
      {shouldShowUpdateActionEntry && (
        <div className="side-nav-update-entry">
          <button
            type="button"
            className={`side-nav-update-btn is-${updateVisualState}`}
            onClick={onUpdateActionClick}
            title={updateEntryTitle}
            disabled={updateActionState === 'installing'}
          >
            {updateActionState === 'downloading' ? (
              <span className="side-nav-update-progress-lr">
                <span
                  className={`side-nav-update-progress-fill${clampedUpdateProgress >= 100 ? ' is-full' : ''}`}
                  style={{ width: `${clampedUpdateProgress}%` }}
                >
                  <span className="side-nav-update-progress-ripple side-nav-update-progress-ripple-a" />
                  <span className="side-nav-update-progress-ripple side-nav-update-progress-ripple-b" />
                </span>
                <span className="side-nav-update-progress-percent">{clampedUpdateProgress}%</span>
              </span>
            ) : updateActionState === 'installing' ? (
              <span className="side-nav-update-text">{t('nav.quickUpdate.installing', '安装中')}</span>
            ) : (
              <span className="side-nav-update-text">
                {updateEntryText}
              </span>
            )}
          </button>
        </div>
      )}

      <div className="nav-brand" style={{ position: 'relative', zIndex: 10 }}>
        <div className="side-nav-brand-main">
          <div
            ref={logoRef}
            className={`brand-logo rocket-easter-egg${hasBreakoutSession ? ' rocket-easter-egg-active' : ''}`}
            onClick={handleLogoClick}
            title={hasBreakoutSession ? t('breakout.resumeGameNav', '继续游戏') : undefined}
          >
            <Rocket size={20} />
            {hasBreakoutSession && <span className="rocket-session-indicator" aria-hidden="true" />}
            {!hasBreakoutSession && easterEggClickCount > 0 && (
              <span className="rocket-click-count">{easterEggClickCount}</span>
            )}
          </div>

          {isClassicLayout && !isClassicCollapsed && (
            <div className="side-nav-brand-title">{APP_DISPLAY_NAME}</div>
          )}
        </div>

        <div
          style={{
            position: 'absolute',
            top: 0,
            left: 0,
            width: '100%',
            height: '100%',
            pointerEvents: 'none',
          }}
        >
          {flyingRockets.map((rocket) => (
            <span
              key={rocket.id}
              className="flying-rocket"
              style={{ '--rocket-x': `${rocket.x}px` } as CSSProperties}
            >
              🚀
            </span>
          ))}
        </div>

      </div>

      <div className="nav-items">
        <button
          className={`nav-item ${page === 'dashboard' && !shouldLockActiveOnMore ? 'active' : ''}`}
          onClick={() => setPage('dashboard')}
          title={t('nav.dashboard')}
        >
          <GaugeCircle size={20} />
          {showClassicLabels ? (
            <span className="nav-item-text">{t('nav.dashboard')}</span>
          ) : !isClassicLayout ? (
            <span className="tooltip">{t('nav.dashboard')}</span>
          ) : null}
        </button>

        {sidebarMenuEntries.map((entry) => {
          const active = currentEntryId === entry.id && !shouldLockActiveOnMore;
          return (
            <button
              key={entry.id}
              className={`nav-item ${active ? 'active' : ''}`}
              onClick={() => navigateToEntry(entry)}
              title={entry.kind === 'api-relay' && campaignActive ? `${entry.label} · ${t('apiKeyFun.campaign.title')}` : entry.label}
            >
              {renderEntryIcon(entry, 20)}
              {entry.kind === 'api-relay' && campaignActive && (
                <span className={`api-relay-campaign-badge ${showClassicLabels ? 'with-label' : 'icon-badge'}`}>
                  {t('apiKeyFun.campaign.badge')}
                </span>
              )}
              {showClassicLabels ? (
                <span className="nav-item-text">{entry.label}</span>
              ) : !isClassicLayout ? (
                <span className="tooltip">{entry.label}</span>
              ) : null}
            </button>
          );
        })}

        <button
          ref={moreButtonRef}
          className={`nav-item ${showMore || isMoreActive ? 'active' : ''}`}
          onClick={() => setShowMore((prev) => !prev)}
          title={t('nav.morePlatforms', '更多平台')}
        >
          <LayoutGrid size={20} />
          {showClassicLabels ? (
            <span className="nav-item-text">{t('nav.morePlatforms', '更多平台')}</span>
          ) : !isClassicLayout ? (
            <span className="tooltip">{t('nav.morePlatforms', '更多平台')}</span>
          ) : null}
        </button>

        {morePopoverContent && (
          typeof document !== 'undefined'
            ? createPortal(morePopoverContent, document.body)
            : morePopoverContent
        )}

      </div>

      {isClassicLayout && (
        <div className="nav-bottom-actions">


          <button
            className={`nav-item ${page === '2fa' && !shouldLockActiveOnMore ? 'active' : ''}`}
            onClick={() => setPage('2fa')}
            title={t('nav.2faManager', '2FA / MFA 管理')}
          >
            <ShieldCheck size={20} />
            {showClassicLabels ? (
              <span className="nav-item-text">{t('nav.2faManager', '2FA / MFA 管理')}</span>
            ) : null}
          </button>

          <button
            className="nav-item"
            onClick={onOpenLogViewer}
            title={t('nav.logs', '日志')}
          >
            <FileText size={20} />
            {showClassicLabels ? (
              <span className="nav-item-text">{t('nav.logs', '日志')}</span>
            ) : null}
          </button>

          <button
            className={`nav-item ${page === 'settings' && !shouldLockActiveOnMore ? 'active' : ''}`}
            onClick={() => setPage('settings')}
            title={t('nav.settings')}
          >
            <Settings size={20} />
            {showClassicLabels ? (
              <span className="nav-item-text">{t('nav.settings')}</span>
            ) : null}
          </button>
        </div>
      )}

      {!isClassicLayout && (
        <>
          <div className="nav-footer">
            <button
              className={`nav-item ${page === '2fa' && !shouldLockActiveOnMore ? 'active' : ''}`}
              onClick={() => setPage('2fa')}
              title={t('nav.2faManager', '2FA / MFA 管理')}
            >
              <ShieldCheck size={20} />
              <span className="tooltip">{t('nav.2faManager', '2FA / MFA 管理')}</span>
            </button>
            <button
              className={`nav-item ${page === 'settings' && !shouldLockActiveOnMore ? 'active' : ''}`}
              onClick={() => setPage('settings')}
              title={t('nav.settings')}
            >
              <Settings size={20} />
              <span className="tooltip">{t('nav.settings')}</span>
            </button>
          </div>

          <button
            type="button"
            className="side-nav-layout-switch-trigger"
            onClick={handleClassicLayoutEntryClick}
            aria-label={t('nav.switchClassicLayoutEntry', '切换到经典布局')}
          >
            <ChevronDown size={16} />
            <span className="tooltip">{t('nav.switchClassicLayoutEntry', '切换到经典布局')}</span>
          </button>
        </>
      )}

      </nav>

      {isClassicLayout && (
        <button
          type="button"
          className={`side-nav-classic-handle${isClassicCollapsed ? ' side-nav-classic-handle-collapsed' : ''}`}
          onClick={toggleClassicCollapsed}
          style={classicHandleStyle}
          title={
            classicCollapsed
              ? t('nav.expandSidebar', '展开侧边栏')
              : t('nav.collapseSidebar', '收起侧边栏')
          }
          aria-label={
            classicCollapsed
              ? t('nav.expandSidebar', '展开侧边栏')
              : t('nav.collapseSidebar', '收起侧边栏')
          }
        >
          {classicCollapsed
            ? <PanelLeftOpen size={16} />
            : <PanelLeftClose size={16} />}
        </button>
      )}
    </>
  );
}
