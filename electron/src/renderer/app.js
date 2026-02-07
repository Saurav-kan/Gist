// Window Controls
const minimizeBtn = document.querySelector(".window-control.minimize");
const maximizeBtn = document.querySelector(".window-control.maximize");
const closeBtn = document.querySelector(".window-control.close");

if (minimizeBtn)
  minimizeBtn.addEventListener("click", () =>
    window.electronAPI.windowMinimize(),
  );
if (maximizeBtn)
  maximizeBtn.addEventListener("click", () =>
    window.electronAPI.windowMaximize(),
  );
if (closeBtn)
  closeBtn.addEventListener("click", () => window.electronAPI.windowClose());

// Sidebar Navigation
document.querySelectorAll(".nav-item").forEach((item) => {
  item.addEventListener("click", () => {
    const page = item.dataset.page;

    // Close preview panel when switching tabs
    closePreviewPanel();

    // Update active nav item
    document
      .querySelectorAll(".nav-item")
      .forEach((n) => n.classList.remove("active"));
    item.classList.add("active");

    // Show corresponding page
    document
      .querySelectorAll(".page")
      .forEach((p) => p.classList.remove("active"));
    const targetPage = document.getElementById(`${page}-page`);
    if (targetPage) {
      targetPage.classList.add("active");
    }

    // Update breadcrumb
    const currentFolder = document.getElementById("current-folder");
    if (currentFolder) {
      currentFolder.textContent =
        page.charAt(0).toUpperCase() + page.slice(1).replace("-", " ");
    }

    // Load data when switching pages
    if (page === "indexing") {
      loadSettings();
      loadSystemInfo();
      loadLibrariesTable();
    } else if (
      page === "desktop" ||
      page === "downloads" ||
      page === "documents" ||
      page === "other-files"
    ) {
      // Load folder page - ensure special folders are loaded first
      if (Object.keys(specialFolders).length === 0) {
        loadSpecialFolders().then(() => loadFolderPage(page));
      } else {
        loadFolderPage(page);
      }
    }
  });
});

// Universal View Preferences (stored in localStorage)
let viewPreferences = {
  view: "grid", // 'grid' or 'list'
  size: "medium", // 'small', 'medium', or 'large'
  searchLayout: "layout-two-row", // 'layout-two-row' or 'layout-compact'
};

// Load view preferences from localStorage
function loadViewPreferences() {
  try {
    const stored = localStorage.getItem("viewPreferences");
    if (stored) {
      viewPreferences = JSON.parse(stored);
    }
  } catch (error) {
    console.error("Failed to load view preferences:", error);
  }
}

// Save view preferences to localStorage
function saveViewPreferences() {
  try {
    localStorage.setItem("viewPreferences", JSON.stringify(viewPreferences));
  } catch (error) {
    console.error("Failed to save view preferences:", error);
  }
}

// Apply view preferences to all file lists
function applyViewPreferences() {
  // Update all view buttons
  document.querySelectorAll(".view-btn").forEach((btn) => {
    if (btn.dataset.view === viewPreferences.view) {
      btn.classList.add("active");
    } else {
      btn.classList.remove("active");
    }
  });

  // Update all size selects
  document.querySelectorAll(".view-size-select").forEach((select) => {
    select.value = viewPreferences.size;
  });

  // Update search layout select
  const layoutSelect = document.getElementById("search-layout-select");
  if (layoutSelect) {
    layoutSelect.value = viewPreferences.searchLayout;
  }

  // Apply search layout
  applySearchLayout();

  // Apply to all file lists
  applyViewToAllLists();
}

// Apply the current search layout to the search page
function applySearchLayout() {
  const searchHeader = document.getElementById("search-header");
  if (searchHeader) {
    searchHeader.classList.remove("layout-two-row", "layout-compact");
    searchHeader.classList.add(viewPreferences.searchLayout);

    // If switching back to two-row, ensure filters panel is not stuck in compact state
    const filtersPanel = document.getElementById("search-filters-panel");
    if (filtersPanel && viewPreferences.searchLayout === "layout-two-row") {
      filtersPanel.classList.remove("visible");
    }
  }
}

// Apply current view settings to all file lists
function applyViewToAllLists() {
  const allFileLists = [
    document.getElementById("results-list"),
    document.getElementById("desktop-file-list"),
    document.getElementById("downloads-file-list"),
    document.getElementById("documents-file-list"),
    document.getElementById("other-files-file-list"),
  ].filter((el) => el !== null);

  allFileLists.forEach((list) => {
    // Apply view type
    if (viewPreferences.view === "list") {
      list.classList.add("list-view");
      list.classList.remove("grid-view");
    } else {
      list.classList.add("grid-view");
      list.classList.remove("list-view");
    }

    // Apply size
    list.classList.remove("size-small", "size-medium", "size-large");
    list.classList.add(`size-${viewPreferences.size}`);

    // Update grid template columns for list view
    if (viewPreferences.view === "list") {
      list.style.gridTemplateColumns = "1fr";
    } else {
      // Grid view - adjust based on size
      const minWidths = {
        small: "120px",
        medium: "180px",
        large: "240px",
      };
      list.style.gridTemplateColumns = `repeat(auto-fill, minmax(${minWidths[viewPreferences.size]}, 1fr))`;
    }
  });

  // Re-render current page if needed
  // Use double requestAnimationFrame to ensure DOM updates and styles are fully applied before re-rendering
  requestAnimationFrame(() => {
    requestAnimationFrame(() => {
      const activePage = document.querySelector(".page.active");
      if (activePage) {
        const pageId = activePage.id;
        if (pageId === "search-page" && lastSearchResults.length > 0) {
          displayResults(lastSearchResults);
        } else if (
          [
            "desktop-page",
            "downloads-page",
            "documents-page",
            "other-files-page",
          ].includes(pageId)
        ) {
          // Reload current folder page
          const pageType = pageId.replace("-page", "");
          loadFolderPage(pageType);
        }
      }
    });
  });
}

// View Toggle (List/Grid) - Universal
document.addEventListener("click", (e) => {
  if (e.target.closest(".view-btn")) {
    const btn = e.target.closest(".view-btn");
    viewPreferences.view = btn.dataset.view;

    // Update all view buttons
    document.querySelectorAll(".view-btn").forEach((b) => {
      if (b.dataset.view === viewPreferences.view) {
        b.classList.add("active");
      } else {
        b.classList.remove("active");
      }
    });

    saveViewPreferences();
    applyViewToAllLists();
  }
});

// Size Select - Universal
document.addEventListener("change", (e) => {
  if (e.target.classList.contains("view-size-select")) {
    viewPreferences.size = e.target.value;

    // Update all size selects
    document.querySelectorAll(".view-size-select").forEach((select) => {
      select.value = viewPreferences.size;
    });

    saveViewPreferences();
    applyViewToAllLists();
  }
});

// Search Layout Select
document.addEventListener("change", (e) => {
  if (e.target.id === "search-layout-select") {
    viewPreferences.searchLayout = e.target.value;
    saveViewPreferences();
    applySearchLayout();
  }
});

// Toggle Filter Panel (Compact Layout only)
document.addEventListener("click", (e) => {
  if (e.target.closest("#toggle-filters-btn")) {
    const filtersPanel = document.getElementById("search-filters-panel");
    if (filtersPanel) {
      filtersPanel.classList.toggle("visible");

      // Add active state to button
      const btn = e.target.closest("#toggle-filters-btn");
      btn.classList.toggle(
        "active",
        filtersPanel.classList.contains("visible"),
      );
    }
  }
});

// Similarity Slider
const similaritySlider = document.getElementById("similarity-slider");
const similarityValue = document.getElementById("similarity-value");
let similarityThreshold = 30; // Lower default threshold for better results

if (similaritySlider) {
  similaritySlider.addEventListener("input", (e) => {
    similarityThreshold = parseInt(e.target.value);
    if (similarityValue)
      similarityValue.textContent = `${similarityThreshold}%`;

    // Filter results if we have them
    if (lastSearchResults.length > 0) {
      filterResultsBySimilarity();
    }
  });
}

function filterResultsBySimilarity() {
  const filtered = lastSearchResults.filter(
    (r) => r.similarity * 100 >= similarityThreshold,
  );
  displayResults(filtered);
}

// Sort Select
const sortSelect = document.getElementById("sort-select");
if (sortSelect) {
  sortSelect.addEventListener("change", (e) => {
    if (lastSearchResults.length > 0) {
      sortResults(e.target.value);
    }
  });
}

function sortResults(sortBy) {
  const sorted = [...lastSearchResults];

  switch (sortBy) {
    case "relevance":
      sorted.sort((a, b) => b.similarity - a.similarity);
      break;
    case "date":
      // Currently backend doesn't return date, so we'll just keep order or mock it
      break;
    case "type":
      sorted.sort((a, b) => {
        const extA = getFileExtension(a.file_path).toLowerCase();
        const extB = getFileExtension(b.file_path).toLowerCase();
        return extA.localeCompare(extB);
      });
      break;
  }

  displayResults(sorted);
}

// Search History functionality
let searchHistory = [];

// Load search history from localStorage
function loadSearchHistory() {
  try {
    const stored = localStorage.getItem("searchHistory");
    if (stored) {
      searchHistory = JSON.parse(stored);
    }
  } catch (error) {
    console.error("Failed to load search history:", error);
  }
}

// Save search history to localStorage
function saveSearchHistory() {
  try {
    localStorage.setItem("searchHistory", JSON.stringify(searchHistory));
  } catch (error) {
    console.error("Failed to save search history:", error);
  }
}

// Add to search history
function addToSearchHistory(query) {
  if (!query || query.trim().length === 0) return;

  // Remove if already exists
  searchHistory = searchHistory.filter(
    (q) => q.toLowerCase() !== query.toLowerCase(),
  );

  // Add to front
  searchHistory.unshift(query.trim());

  // Keep only last 10
  searchHistory = searchHistory.slice(0, 10);

  saveSearchHistory();
}

// Remove from search history
function removeFromSearchHistory(query) {
  searchHistory = searchHistory.filter(
    (q) => q.toLowerCase() !== query.toLowerCase(),
  );
  saveSearchHistory();
  // Refresh the dropdown if it's visible
  if (
    document.getElementById("search-history-dropdown")?.style.display ===
    "block"
  ) {
    showSearchHistory();
  }
}

// Show search history dropdown (portal to body for overflow visibility)
function showSearchHistory() {
  if (!searchInput || searchHistory.length === 0) return;

  // Create dropdown if it doesn't exist
  let dropdown = document.getElementById("search-history-dropdown");
  if (!dropdown) {
    dropdown = document.createElement("div");
    dropdown.id = "search-history-dropdown";
    dropdown.className = "search-history-dropdown";
    document.body.appendChild(dropdown);
  }

  // Position below search input using getBoundingClientRect (escapes overflow:hidden ancestors)
  const rect = searchInput.getBoundingClientRect();
  dropdown.style.position = "fixed";
  dropdown.style.left = `${rect.left}px`;
  dropdown.style.top = `${rect.bottom + 4}px`;
  dropdown.style.width = `${rect.width}px`;
  dropdown.style.minWidth = `${rect.width}px`;

  dropdown.innerHTML = searchHistory
    .map(
      (query, idx) => `
        <div class="history-item" data-query="${escapeHtml(query)}">
            <div class="history-item-content">
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="11" cy="11" r="8"></circle><line x1="21" y1="21" x2="16.65" y2="16.65"></line></svg>
                <span>${escapeHtml(query)}</span>
            </div>
            <button class="history-item-delete" data-query="${escapeHtml(query)}" title="Remove from history">
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="18" y1="6" x2="6" y2="18"></line><line x1="6" y1="6" x2="18" y2="18"></line></svg>
            </button>
        </div>
    `,
    )
    .join("");

  dropdown.style.display = "block";

  // Add click handlers for selecting history item
  dropdown.querySelectorAll(".history-item-content").forEach((content) => {
    content.addEventListener("click", (e) => {
      e.stopPropagation();
      const item = content.closest(".history-item");
      const query = item.dataset.query;
      searchInput.value = query;
      dropdown.style.display = "none";
      performSearch();
    });
  });

  // Add click handlers for delete buttons
  dropdown.querySelectorAll(".history-item-delete").forEach((deleteBtn) => {
    deleteBtn.addEventListener("mousedown", (e) => {
      e.preventDefault(); // Prevent blur on search input
      e.stopPropagation();
    });
    deleteBtn.addEventListener("click", (e) => {
      e.stopPropagation();
      e.preventDefault();
      const query = deleteBtn.dataset.query;
      removeFromSearchHistory(query);
      // Keep focus on search input to prevent dropdown from closing
      requestAnimationFrame(() => {
        if (searchInput) {
          searchInput.focus();
        }
      });
    });
  });
}

// Hide search history dropdown
function hideSearchHistory() {
  const dropdown = document.getElementById("search-history-dropdown");
  if (dropdown) {
    setTimeout(() => {
      dropdown.style.display = "none";
    }, 200);
  }
}

// Search functionality
const searchInput = document.getElementById("search-input");
const resultsList = document.getElementById("results-list");
const initialState = document.getElementById("initial-state");
const loadingState = document.getElementById("loading-state");
const noResults = document.getElementById("no-results");
const resultsCount = document.getElementById("results-count");
let lastSearchResults = [];

if (searchInput) {
  searchInput.addEventListener("keypress", (e) => {
    if (e.key === "Enter") {
      performSearch();
      hideSearchHistory();
    }
  });

  searchInput.addEventListener("focus", () => {
    showSearchHistory();
  });

  searchInput.addEventListener("blur", (e) => {
    // Don't hide dropdown if focus is moving to an element within the dropdown
    const dropdown = document.getElementById("search-history-dropdown");
    const relatedTarget = e.relatedTarget;
    if (dropdown && relatedTarget && dropdown.contains(relatedTarget)) {
      return; // Focus is moving to dropdown, keep it open
    }
    hideSearchHistory();
  });

  searchInput.addEventListener("input", () => {
    if (searchInput.value.trim().length === 0) {
      hideSearchHistory();
    }
  });
}

// Current active filters
let activeFilters = null;

async function performSearch() {
  const query = searchInput.value.trim();
  if (!query) return;

  // Add to search history
  addToSearchHistory(query);

  // Hide all but loading
  if (resultsList) resultsList.style.display = "none";
  if (initialState) initialState.style.display = "none";
  if (noResults) noResults.style.display = "none";
  if (loadingState) loadingState.style.display = "flex";
  if (resultsCount) resultsCount.textContent = "Searching...";

  try {
    // First, parse the query to extract filters
    let parsedQuery = null;
    try {
      const parseResponse = await window.electronAPI.apiRequest(
        "POST",
        "/api/search/parse",
        {
          query,
        },
      );
      if (parseResponse.success && parseResponse.data) {
        // Defensive structure check: Handle both wrapped and direct structures
        parsedQuery = parseResponse.data.data
          ? parseResponse.data.data
          : parseResponse.data;

        // Ensure query field exists and is a string
        if (!parsedQuery.query || typeof parsedQuery.query !== "string") {
          console.warn("Parser returned invalid query structure:", parsedQuery);
          // Check for nested fields if structure changed
          parsedQuery.query = parsedQuery.search_query || query;
        }

        activeFilters = parsedQuery.filters;
        // Display filters in UI
        displayFilters(parsedQuery.filters);
      }
    } catch (parseError) {
      console.warn("Failed to parse query:", parseError);
      // Continue with original query if parsing fails
      activeFilters = null;
      displayFilters(null);
    }

    // Perform search with parsed query and filters
    // Safely extract query with proper null/undefined checks
    let searchQuery = null;
    if (parsedQuery && parsedQuery.query) {
      searchQuery = parsedQuery.query.trim();
    } else if (query) {
      searchQuery = query.trim();
    }

    // Validate query is not empty
    if (!searchQuery) {
      console.error("Search query is empty after parsing!", {
        parsedQuery,
        originalQuery: query,
      });
      showToast("Search query cannot be empty", "error");
      if (loadingState) loadingState.style.display = "none";
      if (initialState) initialState.style.display = "flex";
      return;
    }

    // Only send filters if they actually have values (not all null/undefined)
    let filtersToSend = null;
    if (activeFilters) {
      const hasAnyFilters =
        activeFilters.date_range ||
        (activeFilters.file_types && activeFilters.file_types.length > 0) ||
        (activeFilters.folder_paths && activeFilters.folder_paths.length > 0);
      if (hasAnyFilters) {
        filtersToSend = activeFilters;
      }
    }

    console.log(
      "Searching with query:",
      searchQuery,
      "filters:",
      filtersToSend,
    );

    // Check if Action Search is enabled
    if (actionSearchEnabled) {
      // Use the search query as both the retrieval query and the analysis question
      // The backend will decompose it into intent/content automatically.
      performActiveRagSearch(searchQuery, searchQuery);
    } else {
      // Normal search
      const response = await window.electronAPI.apiRequest(
        "POST",
        "/api/search",
        {
          query: searchQuery,
          limit: maxSearchResults,
          filters: filtersToSend,
        },
      );

      if (response.success && response.data) {
        // Handle both response.data.results (if wrapped) and response.data directly
        const results = response.data.results || response.data;

        if (Array.isArray(results)) {
          lastSearchResults = results;
          console.log(
            "Search results received:",
            lastSearchResults.length,
            "results",
          );
          if (lastSearchResults.length > 0) {
            console.log(
              "Sample similarities:",
              lastSearchResults.slice(0, 5).map((r) => ({
                file: r.file_name,
                similarity: (r.similarity * 100).toFixed(1) + "%",
              })),
            );
            console.log("Similarity threshold:", similarityThreshold + "%");
          } else {
            console.warn("No results returned from backend");
          }
          if (resultsCount)
            resultsCount.textContent = `Found ${lastSearchResults.length} relevant documents`;
          filterResultsBySimilarity();

          // Log after filtering
          const displayedResults =
            document.querySelectorAll(".result-item").length;
          console.log(
            `After similarity filtering: ${displayedResults} results displayed (threshold: ${similarityThreshold}%)`,
          );
        } else {
          console.error("Invalid search response format:", response.data);
          showError("Search failed: Invalid response format");
          if (loadingState) loadingState.style.display = "none";
          if (initialState) initialState.style.display = "flex";
        }
      } else {
        const errorMsg =
          response.error || response.data?.error || "Unknown error";
        console.error("Search failed:", errorMsg, response);
        showError("Search failed: " + errorMsg);
        if (loadingState) loadingState.style.display = "none";
        if (initialState) initialState.style.display = "flex";
      }
    }
  } catch (error) {
    const errorMsg = error?.message || error?.toString() || "Unknown error";
    console.error("Search error details:", error);
    showError("Search error: " + errorMsg);
    if (loadingState) loadingState.style.display = "none";
    if (initialState) initialState.style.display = "flex";
  }
}

// Display active filters as chips
function displayFilters(filters) {
  const filterContainer = document.getElementById("filter-container");
  const filterChips = document.getElementById("filter-chips");

  if (!filterContainer || !filterChips) return;

  if (
    !filters ||
    (!filters.date_range && !filters.file_types && !filters.folder_paths)
  ) {
    filterContainer.style.display = "none";
    filterChips.innerHTML = "";
    return;
  }

  filterContainer.style.display = "flex";
  filterChips.innerHTML = "";

  // Date filter chip
  if (filters.date_range) {
    const dateRange = filters.date_range;
    let dateText = "";
    if (dateRange.month && dateRange.year) {
      const monthNames = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
      ];
      dateText = `${monthNames[dateRange.month - 1]} ${dateRange.year}`;
    } else if (dateRange.year) {
      dateText = `${dateRange.year}`;
    } else if (dateRange.start && dateRange.end) {
      const startDate = new Date(dateRange.start * 1000);
      const endDate = new Date(dateRange.end * 1000);
      dateText = `${startDate.toLocaleDateString()} - ${endDate.toLocaleDateString()}`;
    }

    if (dateText) {
      const chip = createFilterChip("date", dateText, () => {
        if (activeFilters) {
          activeFilters.date_range = null;
          displayFilters(activeFilters);
          performSearch();
        }
      });
      filterChips.appendChild(chip);
    }
  }

  // File type filter chips
  if (filters.file_types && filters.file_types.length > 0) {
    filters.file_types.forEach((fileType) => {
      const chip = createFilterChip("file-type", fileType.toUpperCase(), () => {
        if (activeFilters && activeFilters.file_types) {
          activeFilters.file_types = activeFilters.file_types.filter(
            (ft) => ft !== fileType,
          );
          if (activeFilters.file_types.length === 0) {
            activeFilters.file_types = null;
          }
          displayFilters(activeFilters);
          performSearch();
        }
      });
      filterChips.appendChild(chip);
    });
  }

  // Folder path filter chips
  if (filters.folder_paths && filters.folder_paths.length > 0) {
    filters.folder_paths.forEach((folder) => {
      const chip = createFilterChip("folder", folder, () => {
        if (activeFilters && activeFilters.folder_paths) {
          activeFilters.folder_paths = activeFilters.folder_paths.filter(
            (fp) => fp !== folder,
          );
          if (activeFilters.folder_paths.length === 0) {
            activeFilters.folder_paths = null;
          }
          displayFilters(activeFilters);
          performSearch();
        }
      });
      filterChips.appendChild(chip);
    });
  }
}

// Create a filter chip element
function createFilterChip(type, label, onRemove) {
  const chip = document.createElement("div");
  chip.className = `filter-chip filter-chip-${type}`;
  chip.innerHTML = `
    <span class="filter-chip-label">${escapeHtml(label)}</span>
    <button class="filter-chip-remove" title="Remove filter">×</button>
  `;

  chip.querySelector(".filter-chip-remove").addEventListener("click", (e) => {
    e.stopPropagation();
    onRemove();
  });

  return chip;
}

// Clear all filters
const clearFiltersBtn = document.getElementById("clear-filters-btn");
if (clearFiltersBtn) {
  clearFiltersBtn.addEventListener("click", () => {
    activeFilters = null;
    displayFilters(null);
    performSearch();
  });
}

async function displayResults(results) {
  if (!resultsList) return;

  // Hide loading
  if (loadingState) loadingState.style.display = "none";

  if (results.length === 0) {
    if (noResults) noResults.style.display = "flex";
    resultsList.style.display = "none";
    return;
  }

  if (noResults) noResults.style.display = "none";
  if (initialState) initialState.style.display = "none";
  resultsList.style.display = "grid";

  // Apply view preferences BEFORE clearing HTML to avoid reflow issues
  applyViewToElement(resultsList);

  // Clear HTML after styles are applied
  resultsList.innerHTML = "";

  for (const result of results) {
    const item = document.createElement("div");
    item.className = "result-item";

    const filePath = result.file_path;
    const fileName = result.file_name || filePath.split(/[\\/]/).pop();
    const fileIconData = getFileIcon(fileName);

    // Truncate file name and path for display
    const displayFileName = truncateFileName(fileName, 40);
    const displayFilePath = truncateFilePath(filePath, 50);

    // Get file preview/description
    let description = "";
    try {
      const previewResult = await window.electronAPI.readFilePreview(filePath);
      if (previewResult.success && previewResult.isText) {
        const lines = previewResult.preview.split("\n").filter((l) => l.trim());
        description = lines[0]
          ? lines[0].substring(0, 120)
          : previewResult.preview.substring(0, 120);
        if (description.length >= 120) description += "...";
      }
    } catch (error) {
      // Preview failed, continue
    }

    item.innerHTML = `
      <div class="result-header">
        <div class="file-icon-wrapper" data-file-type="${fileIconData.category}" data-file-path="${escapeHtml(filePath)}">${fileIconData.icon}</div>
        <div class="file-info">
          <div class="file-name" title="${escapeHtml(fileName)}">${escapeHtml(displayFileName)}</div>
          ${description ? `<div class="file-preview">${escapeHtml(description)}</div>` : ""}
        </div>
      </div>
      <div class="result-footer">
        <div class="file-path-tag" title="${escapeHtml(filePath)}">${escapeHtml(displayFilePath)}</div>
        <div class="relevance-tag">${(result.similarity * 100).toFixed(0)}% Match</div>
      </div>
    `;

    // Enhance .exe icon asynchronously if available
    if (fileName.toLowerCase().endsWith(".exe")) {
      enhanceExeIcon(item.querySelector(".file-icon-wrapper"), filePath);
    }

    // Single-click: preview. Double-click: open file externally (same as Desktop/Downloads)
    let clickTimer = null;
    item.addEventListener("click", async () => {
      if (clickTimer) {
        clearTimeout(clickTimer);
        clickTimer = null;
        return;
      }
      clickTimer = setTimeout(async () => {
        clickTimer = null;
        await openPreviewPanel(filePath);
      }, 200);
    });
    item.addEventListener("dblclick", async (e) => {
      e.preventDefault();
      if (clickTimer) {
        clearTimeout(clickTimer);
        clickTimer = null;
      }
      await openFile(filePath);
    });

    resultsList.appendChild(item);
  }
}

function getFileExtension(filename) {
  if (!filename) return "file";
  const parts = filename.split(".");
  return parts.length > 1 ? parts[parts.length - 1] : "file";
}

// Enhance .exe icon asynchronously if custom icon is available
async function enhanceExeIcon(iconElement, filePath) {
  if (!iconElement || !filePath) return;

  try {
    const result = await window.electronAPI.getFileIcon(filePath);
    if (result.success && result.iconPath) {
      iconElement.innerHTML = `<img src="${result.iconPath}" alt="icon" style="width: 24px; height: 24px;" />`;
    }
  } catch (error) {
    // Icon extraction failed, keep default icon
    console.debug("Failed to extract .exe icon:", error);
  }
}

// Get appropriate icon SVG and file type category for file type
function getFileIcon(filename, isDirectory = false) {
  if (isDirectory) {
    return {
      icon: `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"></path></svg>`,
      category: "folder",
    };
  }

  const ext = getFileExtension(filename).toLowerCase();

  // Default executable icon for .exe files (can be enhanced asynchronously)
  if (filename.toLowerCase().endsWith(".exe")) {
    return {
      icon: `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"></path><polyline points="14 2 14 8 20 8"></polyline><line x1="16" y1="13" x2="8" y2="13"></line><line x1="16" y1="17" x2="8" y2="17"></line><polyline points="10 9 9 9 8 9"></polyline></svg>`,
      category: "executable",
    };
  }

  // PDF - Red
  if (ext === "pdf") {
    return {
      icon: `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"></path><polyline points="14 2 14 8 20 8"></polyline><line x1="16" y1="13" x2="8" y2="13"></line><line x1="16" y1="17" x2="8" y2="17"></line><polyline points="10 9 9 9 8 9"></polyline></svg>`,
      category: "pdf",
    };
  }

  // DOC, DOCX, XLS - Blue
  if (["doc", "docx", "xls"].includes(ext)) {
    return {
      icon: `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"></path><polyline points="14 2 14 8 20 8"></polyline><line x1="16" y1="13" x2="8" y2="13"></line><line x1="16" y1="17" x2="8" y2="17"></line><line x1="10" y1="9" x2="8" y2="9"></line></svg>`,
      category: "office",
    };
  }

  // TXT - Grey
  if (ext === "txt") {
    return {
      icon: `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"></path><polyline points="14 2 14 8 20 8"></polyline><line x1="16" y1="13" x2="8" y2="13"></line><line x1="16" y1="17" x2="8" y2="17"></line></svg>`,
      category: "text",
    };
  }

  // Config files - Orange/Yellow (gear/cog icon)
  const configTypes = {
    json: `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3"></circle><path d="M12 1v6m0 6v6M5.64 5.64l4.24 4.24m4.24 4.24l4.24 4.24M1 12h6m6 0h6M5.64 18.36l4.24-4.24m4.24-4.24l4.24-4.24"></path></svg>`,
    yaml: `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3"></circle><path d="M12 1v6m0 6v6M5.64 5.64l4.24 4.24m4.24 4.24l4.24 4.24M1 12h6m6 0h6M5.64 18.36l4.24-4.24m4.24-4.24l4.24-4.24"></path></svg>`,
    yml: `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3"></circle><path d="M12 1v6m0 6v6M5.64 5.64l4.24 4.24m4.24 4.24l4.24 4.24M1 12h6m6 0h6M5.64 18.36l4.24-4.24m4.24-4.24l4.24-4.24"></path></svg>`,
    toml: `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3"></circle><path d="M12 1v6m0 6v6M5.64 5.64l4.24 4.24m4.24 4.24l4.24 4.24M1 12h6m6 0h6M5.64 18.36l4.24-4.24m4.24-4.24l4.24-4.24"></path></svg>`,
    ini: `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3"></circle><path d="M12 1v6m0 6v6M5.64 5.64l4.24 4.24m4.24 4.24l4.24 4.24M1 12h6m6 0h6M5.64 18.36l4.24-4.24m4.24-4.24l4.24-4.24"></path></svg>`,
    cfg: `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3"></circle><path d="M12 1v6m0 6v6M5.64 5.64l4.24 4.24m4.24 4.24l4.24 4.24M1 12h6m6 0h6M5.64 18.36l4.24-4.24m4.24-4.24l4.24-4.24"></path></svg>`,
    conf: `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3"></circle><path d="M12 1v6m0 6v6M5.64 5.64l4.24 4.24m4.24 4.24l4.24 4.24M1 12h6m6 0h6M5.64 18.36l4.24-4.24m4.24-4.24l4.24-4.24"></path></svg>`,
    properties: `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3"></circle><path d="M12 1v6m0 6v6M5.64 5.64l4.24 4.24m4.24 4.24l4.24 4.24M1 12h6m6 0h6M5.64 18.36l4.24-4.24m4.24-4.24l4.24-4.24"></path></svg>`,
    config: `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3"></circle><path d="M12 1v6m0 6v6M5.64 5.64l4.24 4.24m4.24 4.24l4.24 4.24M1 12h6m6 0h6M5.64 18.36l4.24-4.24m4.24-4.24l4.24-4.24"></path></svg>`,
  };

  if (configTypes[ext]) {
    return {
      icon: configTypes[ext],
      category: "config",
    };
  }

  // Other document types
  const documentTypes = {
    xlsx: `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"></path><polyline points="14 2 14 8 20 8"></polyline><line x1="16" y1="13" x2="8" y2="13"></line><line x1="16" y1="17" x2="8" y2="17"></line><line x1="10" y1="9" x2="8" y2="9"></line></svg>`,
    rtf: `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"></path><polyline points="14 2 14 8 20 8"></polyline><line x1="16" y1="13" x2="8" y2="13"></line><line x1="16" y1="17" x2="8" y2="17"></line></svg>`,
  };

  // Code/Text files
  const codeTypes = {
    js: `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="16 18 22 12 16 6"></polyline><polyline points="8 6 2 12 8 18"></polyline></svg>`,
    ts: `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="16 18 22 12 16 6"></polyline><polyline points="8 6 2 12 8 18"></polyline></svg>`,
    jsx: `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="16 18 22 12 16 6"></polyline><polyline points="8 6 2 12 8 18"></polyline></svg>`,
    tsx: `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="16 18 22 12 16 6"></polyline><polyline points="8 6 2 12 8 18"></polyline></svg>`,
    py: `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="16 18 22 12 16 6"></polyline><polyline points="8 6 2 12 8 18"></polyline></svg>`,
    java: `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="16 18 22 12 16 6"></polyline><polyline points="8 6 2 12 8 18"></polyline></svg>`,
    cpp: `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="16 18 22 12 16 6"></polyline><polyline points="8 6 2 12 8 18"></polyline></svg>`,
    c: `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="16 18 22 12 16 6"></polyline><polyline points="8 6 2 12 8 18"></polyline></svg>`,
    rs: `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="16 18 22 12 16 6"></polyline><polyline points="8 6 2 12 8 18"></polyline></svg>`,
    go: `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="16 18 22 12 16 6"></polyline><polyline points="8 6 2 12 8 18"></polyline></svg>`,
    html: `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="16 18 22 12 16 6"></polyline><polyline points="8 6 2 12 8 18"></polyline></svg>`,
    css: `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="16 18 22 12 16 6"></polyline><polyline points="8 6 2 12 8 18"></polyline></svg>`,
    xml: `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="16 18 22 12 16 6"></polyline><polyline points="8 6 2 12 8 18"></polyline></svg>`,
    md: `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"></path><polyline points="14 2 14 8 20 8"></polyline><line x1="16" y1="13" x2="8" y2="13"></line><line x1="16" y1="17" x2="8" y2="17"></line><line x1="10" y1="9" x2="8" y2="9"></line></svg>`,
  };

  // Image types
  const imageTypes = {
    jpg: `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="18" height="18" rx="2" ry="2"></rect><circle cx="8.5" cy="8.5" r="1.5"></circle><polyline points="21 15 16 10 5 21"></polyline></svg>`,
    jpeg: `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="18" height="18" rx="2" ry="2"></rect><circle cx="8.5" cy="8.5" r="1.5"></circle><polyline points="21 15 16 10 5 21"></polyline></svg>`,
    png: `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="18" height="18" rx="2" ry="2"></rect><circle cx="8.5" cy="8.5" r="1.5"></circle><polyline points="21 15 16 10 5 21"></polyline></svg>`,
    gif: `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="18" height="18" rx="2" ry="2"></rect><circle cx="8.5" cy="8.5" r="1.5"></circle><polyline points="21 15 16 10 5 21"></polyline></svg>`,
    svg: `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="18" height="18" rx="2" ry="2"></rect><circle cx="8.5" cy="8.5" r="1.5"></circle><polyline points="21 15 16 10 5 21"></polyline></svg>`,
    webp: `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="18" height="18" rx="2" ry="2"></rect><circle cx="8.5" cy="8.5" r="1.5"></circle><polyline points="21 15 16 10 5 21"></polyline></svg>`,
    bmp: `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="18" height="18" rx="2" ry="2"></rect><circle cx="8.5" cy="8.5" r="1.5"></circle><polyline points="21 15 16 10 5 21"></polyline></svg>`,
    ico: `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="18" height="18" rx="2" ry="2"></rect><circle cx="8.5" cy="8.5" r="1.5"></circle><polyline points="21 15 16 10 5 21"></polyline></svg>`,
  };

  // Media types
  const mediaTypes = {
    mp4: `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="5 3 19 12 5 21 5 3"></polygon></svg>`,
    avi: `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="5 3 19 12 5 21 5 3"></polygon></svg>`,
    mov: `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="5 3 19 12 5 21 5 3"></polygon></svg>`,
    mp3: `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5"></polygon><path d="M19.07 4.93a10 10 0 0 1 0 14.14M15.54 8.46a5 5 0 0 1 0 7.07"></path></svg>`,
    wav: `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5"></polygon><path d="M19.07 4.93a10 10 0 0 1 0 14.14M15.54 8.46a5 5 0 0 1 0 7.07"></path></svg>`,
    flac: `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5"></polygon><path d="M19.07 4.93a10 10 0 0 1 0 14.14M15.54 8.46a5 5 0 0 1 0 7.07"></path></svg>`,
    ogg: `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5"></polygon><path d="M19.07 4.93a10 10 0 0 1 0 14.14M15.54 8.46a5 5 0 0 1 0 7.07"></path></svg>`,
  };

  // Archive types
  const archiveTypes = {
    zip: `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z"></path><polyline points="3.27 6.96 12 12.01 20.73 6.96"></polyline><line x1="12" y1="22.08" x2="12" y2="12"></line></svg>`,
    rar: `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z"></path><polyline points="3.27 6.96 12 12.01 20.73 6.96"></polyline><line x1="12" y1="22.08" x2="12" y2="12"></line></svg>`,
    "7z": `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z"></path><polyline points="3.27 6.96 12 12.01 20.73 6.96"></polyline><line x1="12" y1="22.08" x2="12" y2="12"></line></svg>`,
    tar: `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z"></path><polyline points="3.27 6.96 12 12.01 20.73 6.96"></polyline><line x1="12" y1="22.08" x2="12" y2="12"></line></svg>`,
    gz: `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z"></path><polyline points="3.27 6.96 12 12.01 20.73 6.96"></polyline><line x1="12" y1="22.08" x2="12" y2="12"></line></svg>`,
  };

  // Check each category
  if (documentTypes[ext])
    return { icon: documentTypes[ext], category: "document" };
  if (codeTypes[ext]) return { icon: codeTypes[ext], category: "code" };
  if (imageTypes[ext]) return { icon: imageTypes[ext], category: "image" };
  if (mediaTypes[ext]) return { icon: mediaTypes[ext], category: "media" };
  if (archiveTypes[ext])
    return { icon: archiveTypes[ext], category: "archive" };

  // Default file icon
  return {
    icon: `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"></path><polyline points="14 2 14 8 20 8"></polyline></svg>`,
    category: "file",
  };
}

async function openFile(filePath) {
  try {
    const result = await window.electronAPI.openFile(filePath);
    if (!result.success) {
      showToast(
        "Failed to open file: " + (result.error || "Unknown error"),
        "error",
      );
    }
  } catch (error) {
    showToast("Error opening file: " + error.message, "error");
  }
}

// Open preview panel for a file
async function openPreviewPanel(filePath) {
  const previewPanel = document.getElementById("preview-panel");
  if (!previewPanel) return;

  // Show preview panel
  previewPanel.classList.add("active");

  // Get file name for header
  const fileName = filePath.split(/[\\/]/).pop();
  const previewFileName = document.getElementById("preview-file-name");
  if (previewFileName) {
    previewFileName.textContent = fileName;
    previewFileName.title = filePath;
  }

  // Show loading state
  const previewLoading = document.getElementById("preview-loading");
  const previewText = document.getElementById("preview-text");
  const previewImage = document.getElementById("preview-image");
  const previewPdf = document.getElementById("preview-pdf");
  const previewError = document.getElementById("preview-error");
  const previewUnsupported = document.getElementById("preview-unsupported");

  // Hide all preview sections
  if (previewLoading) previewLoading.style.display = "none";
  if (previewText) previewText.style.display = "none";
  if (previewImage) previewImage.style.display = "none";
  if (previewPdf) previewPdf.style.display = "none";
  if (previewError) previewError.style.display = "none";
  if (previewUnsupported) previewUnsupported.style.display = "none";

  // Store current file path for AI features
  previewPanel.dataset.currentFile = filePath;

  // Set up action buttons
  const openExternalBtn = document.getElementById("preview-open-external-btn");
  const showFolderBtn = document.getElementById("preview-show-folder-btn");

  if (openExternalBtn) {
    openExternalBtn.onclick = () => openFile(filePath);
  }

  if (showFolderBtn) {
    showFolderBtn.onclick = async () => {
      try {
        await window.electronAPI.showInFolder(filePath);
      } catch (error) {
        showToast("Failed to show in folder: " + error.message, "error");
      }
    };
  }

  // Show loading state
  if (previewLoading) previewLoading.style.display = "flex";

  // Get preview from backend
  try {
    const response = await window.electronAPI.apiRequest(
      "GET",
      `/api/preview?path=${encodeURIComponent(filePath)}`,
    );

    // Hide loading
    if (previewLoading) previewLoading.style.display = "none";

    if (response.success && response.data) {
      const previewData = response.data;

      // Check if preview was successful
      if (previewData.success === false) {
        // Backend returned error in response body
        if (previewError) {
          previewError.style.display = "block";
          previewError.textContent =
            previewData.error || "Failed to load preview.";
        }
        return;
      }

      // Hide all preview sections first
      if (previewText) previewText.style.display = "none";
      if (previewImage) previewImage.style.display = "none";
      if (previewPdf) previewPdf.style.display = "none";
      if (previewError) previewError.style.display = "none";
      if (previewUnsupported) previewUnsupported.style.display = "none";

      if (!previewData.preview_available) {
        // Preview not available
        if (previewUnsupported) {
          previewUnsupported.style.display = "block";
          previewUnsupported.textContent =
            previewData.error || "Preview not available for this file type.";
        }
      } else {
        // Render based on file type
        switch (previewData.file_type) {
          case "text":
          case "code":
            if (previewText && previewData.content) {
              previewText.style.display = "block";
              previewText.textContent = previewData.content;
              previewText.className = "preview-text";
              if (previewData.file_type === "code") {
                previewText.classList.add("code-preview");
              }
            }
            break;

          case "pdf":
            // Always use PDF.js to render PDFs as-is (preserves formatting, images, layout)
            await renderPdfWithPdfJs(filePath, previewPdf);
            break;

          case "docx":
            if (previewText && previewData.content) {
              previewText.style.display = "block";
              previewText.textContent = previewData.content;
              previewText.className = "preview-text document-preview";
            }
            break;

          case "image":
            if (previewImage) {
              previewImage.style.display = "flex";
              previewImage.innerHTML = `<img src="file:///${filePath.replace(/\\/g, "/")}" alt="${escapeHtml(fileName)}" />`;
            }
            break;

          default:
            if (previewUnsupported) {
              previewUnsupported.style.display = "block";
              previewUnsupported.textContent =
                "Preview not available for this file type.";
            }
        }
      }

      // Show/hide AI section based on settings
      await updateAiSectionVisibility(filePath);
    } else {
      // Error from backend
      if (previewError) {
        previewError.style.display = "block";
        previewError.textContent = response.error || "Failed to load preview.";
      }
    }
  } catch (error) {
    console.error("Preview error:", error);
    if (previewLoading) previewLoading.style.display = "none";
    if (previewError) {
      previewError.style.display = "block";
      previewError.textContent = "Error loading preview: " + error.message;
    }
  }
}

// Update AI section visibility based on settings
async function updateAiSectionVisibility(filePath) {
  const aiSection = document.getElementById("ai-section");
  if (!aiSection) return;

  try {
    const settingsResponse = await window.electronAPI.apiRequest(
      "GET",
      "/api/settings",
    );
    if (settingsResponse.success && settingsResponse.data) {
      const settings = settingsResponse.data;

      if (settings.ai_features_enabled) {
        aiSection.style.display = "flex";
        // Initialize AI features for this file
        initializeAiFeatures(filePath);
      } else {
        aiSection.style.display = "none";
      }
    } else {
      aiSection.style.display = "none";
    }
  } catch (error) {
    console.error("Failed to check AI settings:", error);
    aiSection.style.display = "none";
  }
}

// Initialize AI features for the current file
let currentAiFilePath = null;
let chatHistory = [];

function initializeAiFeatures(filePath) {
  currentAiFilePath = filePath;
  chatHistory = [];

  // Clear previous chat results
  const chatMessages = document.getElementById("ai-chat-messages");
  if (chatMessages) {
    chatMessages.innerHTML = `
      <div class="ai-chat-welcome">
        <p>Ask anything about this document. The AI has access to the full text content.</p>
      </div>
    `;
  }

  // Show summary suggestion when starting fresh
  updateSummarySuggestionVisibility();

  // Set up event listeners
  setupAiEventListeners();
}

// Hide summary suggestion once chat has any real content (user or assistant messages)
function updateSummarySuggestionVisibility() {
  const summarySuggestion = document.getElementById("ai-summary-suggestion");
  const chatMessages = document.getElementById("ai-chat-messages");
  if (!summarySuggestion || !chatMessages) return;

  const hasContent =
    chatMessages.querySelectorAll(".ai-chat-message").length > 0;
  summarySuggestion.style.display = hasContent ? "none" : "block";
}

function setupAiEventListeners() {
  // Toggle expand/collapse
  const toggleBtn = document.getElementById("ai-toggle-expand");
  const aiSection = document.getElementById("ai-section");
  if (toggleBtn && aiSection && !toggleBtn.dataset.listenerAdded) {
    toggleBtn.dataset.listenerAdded = "true";
    toggleBtn.addEventListener("click", () => {
      aiSection.classList.toggle("collapsed");
      const isCollapsed = aiSection.classList.contains("collapsed");
      toggleBtn.title = isCollapsed
        ? "Expand AI section"
        : "Collapse AI section";
    });
  }

  // Summary suggestion - send "Summarize this document" on click
  const summarySuggestion = document.getElementById("ai-summary-suggestion");
  if (summarySuggestion && !summarySuggestion.dataset.listenerAdded) {
    summarySuggestion.dataset.listenerAdded = "true";
    summarySuggestion.addEventListener("click", async () => {
      const suggestion = summarySuggestion.dataset.suggestion;
      if (suggestion) {
        await sendChatMessage(suggestion);
      }
    });
  }

  // Chat send button
  const chatSendBtn = document.getElementById("ai-chat-send-btn");
  const chatInput = document.getElementById("ai-chat-input");

  if (chatSendBtn && !chatSendBtn.dataset.listenerAdded) {
    chatSendBtn.dataset.listenerAdded = "true";
    chatSendBtn.addEventListener("click", async () => {
      await sendChatMessage();
    });
  }

  if (chatInput && !chatInput.dataset.listenerAdded) {
    chatInput.dataset.listenerAdded = "true";
    chatInput.addEventListener("keypress", async (e) => {
      if (e.key === "Enter") {
        await sendChatMessage();
      }
    });
  }
}

// Send chat message (optional messageOverride: use when sending from suggestion chip)
async function sendChatMessage(messageOverride) {
  if (!currentAiFilePath) return;

  const chatInput = document.getElementById("ai-chat-input");
  const chatSendBtn = document.getElementById("ai-chat-send-btn");
  const chatMessages = document.getElementById("ai-chat-messages");

  if (!chatInput || !chatSendBtn || !chatMessages) return;

  const message = typeof messageOverride === "string"
    ? messageOverride.trim()
    : chatInput.value.trim();
  if (!message) return;

  // Add user message to UI
  addChatMessage("user", message);

  // Clear input
  chatInput.value = "";
  chatInput.disabled = true;
  chatSendBtn.disabled = true;

  // Add loading message
  const loadingId = "loading-" + Date.now();
  addChatMessage("assistant", "", loadingId);
  const loadingElement = document.getElementById(loadingId);
  if (loadingElement) {
    loadingElement.innerHTML = `
      <div class="ai-loading-dots">
        <span></span><span></span><span></span>
      </div>
    `;
  }

  try {
    // Prepare conversation history
    const history = chatHistory.map((msg) => ({
      role: msg.role,
      content: msg.content,
    }));

    const response = await window.electronAPI.apiRequest(
      "POST",
      "/api/ai/chat",
      {
        file_path: currentAiFilePath,
        message: message,
        conversation_history: history,
      },
    );

    // Remove loading message
    const loadingMsg = document.getElementById(loadingId);
    if (loadingMsg) loadingMsg.remove();

    if (response.success && response.data) {
      if (response.data.success && response.data.message) {
        // Add AI response
        addChatMessage("assistant", response.data.message);

        // Update chat history
        chatHistory.push({ role: "user", content: message });
        chatHistory.push({ role: "assistant", content: response.data.message });
      } else {
        addChatMessage(
          "assistant",
          "Error: " + (response.data.error || "Failed to get response"),
          null,
          true,
        );
      }
    } else {
      addChatMessage(
        "assistant",
        "Error: " + (response.error || "Unknown error"),
        null,
        true,
      );
    }
  } catch (error) {
    console.error("Chat error:", error);
    const loadingMsg = document.getElementById(loadingId);
    if (loadingMsg) loadingMsg.remove();
    addChatMessage("assistant", "Error: " + error.message, null, true);
  } finally {
    chatInput.disabled = false;
    chatSendBtn.disabled = false;
    chatInput.focus();
  }
}

// Add chat message to UI (uses formatAnswerWithHighlights for assistant messages)
function addChatMessage(role, content, messageId = null, isError = false) {
  const chatMessages = document.getElementById("ai-chat-messages");
  if (!chatMessages) return;

  const messageDiv = document.createElement("div");
  messageDiv.className = `ai-chat-message ${role}`;
  if (messageId) messageDiv.id = messageId;
  if (isError) messageDiv.style.color = "#ef4444";

  if (role === "assistant" && !isError && content) {
    messageDiv.innerHTML = formatAnswerWithHighlights(content);
  } else {
    messageDiv.textContent = content;
  }
  chatMessages.appendChild(messageDiv);

  // Hide summary suggestion once chat has content
  updateSummarySuggestionVisibility();

  // Scroll to bottom
  chatMessages.scrollTop = chatMessages.scrollHeight;
}

// PDF.js state
let currentPdfDoc = null;
let currentPdfPageNum = 1;

// Render PDF using PDF.js
async function renderPdfWithPdfJs(filePath, container) {
  if (!container) return;

  try {
    // Load PDF.js if not already loaded
    const pdfjsLib = await loadPdfJs();

    // Show loading
    const previewLoading = document.getElementById("preview-loading");
    if (previewLoading) previewLoading.style.display = "flex";

    // Use file:// protocol for Electron
    const fileUrl = `file:///${filePath.replace(/\\/g, "/")}`;

    // Load PDF document
    const loadingTask = pdfjsLib.getDocument({
      url: fileUrl,
      withCredentials: false,
    });

    currentPdfDoc = await loadingTask.promise;
    currentPdfPageNum = 1;

    // Hide loading
    if (previewLoading) previewLoading.style.display = "none";

    // Show PDF container
    container.style.display = "block";

    // Show controls if multiple pages
    const pdfControls = document.getElementById("pdf-controls");
    const pdfPrevBtn = document.getElementById("pdf-prev-page");
    const pdfNextBtn = document.getElementById("pdf-next-page");

    if (pdfControls && currentPdfDoc.numPages > 1) {
      pdfControls.style.display = "flex";
      updatePdfPageInfo();
      if (pdfPrevBtn) pdfPrevBtn.disabled = true; // First page
      if (pdfNextBtn) pdfNextBtn.disabled = currentPdfDoc.numPages === 1;
    } else if (pdfControls) {
      pdfControls.style.display = "none";
    }

    // Render first page
    await renderPdfPage(currentPdfPageNum, container);
  } catch (error) {
    console.error("PDF.js error:", error);
    const previewLoading = document.getElementById("preview-loading");
    if (previewLoading) previewLoading.style.display = "none";

    const previewError = document.getElementById("preview-error");
    if (previewError) {
      previewError.style.display = "block";
      previewError.textContent = "Failed to load PDF: " + error.message;
    }
  }
}

async function renderPdfPage(pageNum, container) {
  if (!currentPdfDoc || !container) return;

  try {
    const page = await currentPdfDoc.getPage(pageNum);
    const canvas = document.getElementById("pdf-canvas");
    if (!canvas) return;

    const context = canvas.getContext("2d");
    const viewport = page.getViewport({ scale: 1.5 });

    // Set canvas dimensions
    canvas.height = viewport.height;
    canvas.width = viewport.width;

    // Render PDF page
    const renderContext = {
      canvasContext: context,
      viewport: viewport,
    };

    await page.render(renderContext).promise;

    // Update button states
    const pdfPrevBtn = document.getElementById("pdf-prev-page");
    const pdfNextBtn = document.getElementById("pdf-next-page");
    if (pdfPrevBtn) pdfPrevBtn.disabled = pageNum === 1;
    if (pdfNextBtn) pdfNextBtn.disabled = pageNum >= currentPdfDoc.numPages;
  } catch (error) {
    console.error("Error rendering PDF page:", error);
    throw error;
  }
}

function updatePdfPageInfo() {
  const pageInfo = document.getElementById("pdf-page-info");
  if (pageInfo && currentPdfDoc) {
    pageInfo.textContent = `Page ${currentPdfPageNum} of ${currentPdfDoc.numPages}`;
  }
}

// Detect programming language from file path
function detectLanguageFromPath(filePath) {
  const ext = filePath.split(".").pop()?.toLowerCase();
  const languageMap = {
    js: "javascript",
    ts: "typescript",
    py: "python",
    rs: "rust",
    java: "java",
    cpp: "cpp",
    c: "c",
    h: "c",
    hpp: "cpp",
    go: "go",
    rb: "ruby",
    php: "php",
    json: "json",
    xml: "xml",
    html: "xml",
    css: "css",
    sh: "bash",
    bash: "bash",
    zsh: "bash",
    yaml: "yaml",
    yml: "yaml",
    md: "markdown",
    toml: "toml",
  };
  return languageMap[ext] || null;
}

// Close preview panel
function closePreviewPanel() {
  const previewPanel = document.getElementById("preview-panel");
  if (previewPanel) {
    previewPanel.classList.remove("active");
  }
}

function escapeHtml(text) {
  if (!text) return "";
  const div = document.createElement("div");
  div.textContent = text;
  return div.innerHTML;
}

// Truncate file path intelligently
function truncateFilePath(filePath, maxLength = 50) {
  if (!filePath || filePath.length <= maxLength) {
    return filePath;
  }

  // Try to show beginning and end of path
  const parts = filePath.split(/[\\/]/);
  if (parts.length > 2) {
    // Show first part, ellipsis, and last 2 parts
    const firstPart = parts[0];
    const lastParts = parts.slice(-2).join("/");
    const availableLength = maxLength - 3; // 3 for ellipsis
    const firstLength = Math.min(
      firstPart.length,
      Math.floor(availableLength * 0.4),
    );
    const lastLength = Math.min(
      lastParts.length,
      availableLength - firstLength,
    );

    if (firstLength + lastLength < availableLength) {
      return (
        firstPart.substring(0, firstLength) +
        "..." +
        lastParts.substring(lastParts.length - lastLength)
      );
    }
  }

  // Fallback: just truncate from end
  return filePath.substring(0, maxLength - 3) + "...";
}

// Truncate file name if too long
function truncateFileName(fileName, maxLength = 40) {
  if (!fileName || fileName.length <= maxLength) {
    return fileName;
  }

  // Keep extension, truncate name
  const lastDot = fileName.lastIndexOf(".");
  if (lastDot > 0 && lastDot < fileName.length - 1) {
    const name = fileName.substring(0, lastDot);
    const ext = fileName.substring(lastDot);
    const maxNameLength = maxLength - ext.length - 3; // 3 for ellipsis
    if (maxNameLength > 0) {
      return name.substring(0, maxNameLength) + "..." + ext;
    }
  }

  // No extension or can't fit, just truncate
  return fileName.substring(0, maxLength - 3) + "...";
}

function showError(message) {
  console.error(message);
}

function showToast(message, type = "info", duration = 3000) {
  // Create toast element if it doesn't exist
  let toast = document.querySelector(".toast");
  if (!toast) {
    toast = document.createElement("div");
    toast.className = "toast";
    document.body.appendChild(toast);
  }

  // Set message and type
  toast.textContent = message;
  toast.className = `toast ${type}`;

  // Show toast
  toast.style.display = "block";
  toast.style.animation = "slideIn 0.3s ease-out";

  // Hide after duration
  setTimeout(() => {
    toast.style.animation = "slideOut 0.3s ease-out";
    setTimeout(() => {
      toast.style.display = "none";
    }, 300);
  }, duration);
}

// Add toast animations to CSS if not already present
const toastStyles = `
  .toast {
    position: fixed;
    bottom: 2rem;
    right: 2rem;
    padding: 1rem 1.5rem;
    border-radius: 8px;
    color: white;
    font-weight: 500;
    z-index: 1000;
    max-width: 400px;
    word-wrap: break-word;
  }
  
  .toast.success { background: #10b981; }
  .toast.error { background: #ef4444; }
  .toast.warning { background: #f59e0b; }
  .toast.info { background: #3b82f6; }
  
  @keyframes slideIn {
    from {
      transform: translateX(100%);
      opacity: 0;
    }
    to {
      transform: translateX(0);
      opacity: 1;
    }
  }
  
  @keyframes slideOut {
    from {
      transform: translateX(0);
      opacity: 1;
    }
    to {
      transform: translateX(100%);
      opacity: 0;
    }
  }
`;

// Inject styles if not already present
if (!document.querySelector("#toast-styles")) {
  const styleSheet = document.createElement("style");
  styleSheet.id = "toast-styles";
  styleSheet.textContent = toastStyles;
  document.head.appendChild(styleSheet);
}

// Settings functionality
let maxSearchResults = 100; // Default value

async function loadSettings() {
  try {
    const response = await window.electronAPI.apiRequest(
      "GET",
      "/api/settings",
    );
    if (response.success) {
      const settings = response.data;

      // Load performance mode
      const modeRadio = document.querySelector(
        `input[name="performance-mode"][value="${settings.performance_mode}"]`,
      );
      if (modeRadio) {
        modeRadio.checked = true;
        // Update active class on parent card
        document
          .querySelectorAll(".model-card")
          .forEach((card) => card.classList.remove("active"));
        modeRadio.closest(".model-card")?.classList.add("active");
      }

      // Load max search results
      if (settings.max_search_results) {
        maxSearchResults = settings.max_search_results;
        const slider = document.getElementById("max-results-slider");
        const valueDisplay = document.getElementById("max-results-value");
        if (slider) slider.value = maxSearchResults;
        if (valueDisplay) valueDisplay.textContent = maxSearchResults;
      }

      // Load filter duplicate files
      const filterDuplicateCheckbox = document.getElementById(
        "filter-duplicate-files",
      );
      if (
        filterDuplicateCheckbox &&
        settings.filter_duplicate_files !== undefined
      ) {
        filterDuplicateCheckbox.checked = settings.filter_duplicate_files;
      }

      // Load AI settings
      const aiEnabledCheckbox = document.getElementById("ai-features-enabled");
      const aiSettingsContainer = document.getElementById(
        "ai-settings-container",
      );
      const aiProviderSelect = document.getElementById("ai-provider-select");
      const ollamaModelInput = document.getElementById("ollama-model-input");
      const apiKeyInput = document.getElementById("api-key-input");
      const ollamaSettings = document.getElementById("ollama-settings");
      const apiKeySettings = document.getElementById("api-key-settings");
      const geminiSettings = document.getElementById("gemini-settings");

      if (aiEnabledCheckbox) {
        // Explicitly set checkbox state from settings
        const aiEnabled = settings.ai_features_enabled === true;
        aiEnabledCheckbox.checked = aiEnabled;
        console.log("Loading AI settings:", {
          ai_features_enabled: settings.ai_features_enabled,
          checkbox_checked: aiEnabledCheckbox.checked,
          settings_object: settings,
        });
        if (aiSettingsContainer) {
          aiSettingsContainer.style.display = aiEnabled ? "block" : "none";
        }
      }

      if (aiProviderSelect && settings.ai_provider) {
        aiProviderSelect.value = settings.ai_provider;
        updateAiProviderUI(settings.ai_provider);
      }

      if (ollamaModelInput && settings.ollama_model) {
        ollamaModelInput.value = settings.ollama_model;
      }

      if (settings.gemini_model) {
        const geminiModelSelect = document.getElementById(
          "gemini-model-select",
        );
        if (geminiModelSelect) {
          // Store it to be restored after models are loaded
          geminiModelSelect.dataset.savedModel = settings.gemini_model;
        }
      }

      // Don't load API key (security - backend doesn't send it)

      // Load Action Search model settings
      const parsingModelSelect = document.getElementById(
        "action-search-parsing-model",
      );
      const analysisModelSelect = document.getElementById(
        "action-search-analysis-model",
      );

      if (parsingModelSelect && settings.action_search_parsing_model) {
        parsingModelSelect.value = settings.action_search_parsing_model;
      }

      if (analysisModelSelect && settings.action_search_analysis_model) {
        analysisModelSelect.value = settings.action_search_analysis_model;
      }

      // Load excluded extensions
      const excludedInput = document.getElementById(
        "excluded-extensions-input",
      );
      if (
        excludedInput &&
        settings.file_type_filters &&
        settings.file_type_filters.excluded_extensions
      ) {
        excludedInput.value =
          settings.file_type_filters.excluded_extensions.join(", ");
      }
    }
  } catch (error) {
    console.error("Failed to load settings:", error);
  }
}

// ...
async function updateAiProviderUI(provider) {
  const ollamaSettings = document.getElementById("ollama-settings");
  const apiKeySettings = document.getElementById("api-key-settings");
  const geminiSettings = document.getElementById("gemini-settings");

  if (provider === "ollama") {
    if (ollamaSettings) ollamaSettings.style.display = "block";
    if (apiKeySettings) apiKeySettings.style.display = "none";
    if (geminiSettings) geminiSettings.style.display = "none";
  } else if (provider === "gemini") {
    if (ollamaSettings) ollamaSettings.style.display = "none";
    if (apiKeySettings) apiKeySettings.style.display = "block";
    if (geminiSettings) geminiSettings.style.display = "block";
    // Fetch Gemini models when Gemini is selected
    await loadGeminiModels();
  } else {
    if (ollamaSettings) ollamaSettings.style.display = "none";
    if (apiKeySettings) apiKeySettings.style.display = "block";
    if (geminiSettings) geminiSettings.style.display = "none";
  }
}

// Load available Gemini models from API
async function loadGeminiModels() {
  const geminiModelSelect = document.getElementById("gemini-model-select");
  const apiKeyInput = document.getElementById("api-key-input");

  if (!geminiModelSelect || !apiKeyInput) return;

  let apiKey = apiKeyInput.value.trim();

  // If no API key in input, try to see if one is already saved in backend
  // We can't see the key, but the backend can use it if we don't send one
  // Actually, for the model list endpoint, we MUST provide a key currently

  if (!apiKey) {
    geminiModelSelect.innerHTML =
      '<option value="gemini-pro">Enter API key to load models</option>';
    return;
  }

  try {
    geminiModelSelect.innerHTML =
      '<option value="gemini-pro">Loading models...</option>';
    geminiModelSelect.disabled = true;

    const response = await window.electronAPI.apiRequest(
      "GET",
      `/api/ai/gemini-models?api_key=${encodeURIComponent(apiKey)}`,
    );

    if (response.success && response.data && response.data.models) {
      const models = response.data.models;
      geminiModelSelect.innerHTML = "";

      models.forEach((model) => {
        const option = document.createElement("option");
        option.value = model;
        option.textContent = model;
        geminiModelSelect.appendChild(option);
      });

      // Restore saved model if available
      const savedModel = geminiModelSelect.dataset.savedModel;
      if (savedModel && models.includes(savedModel)) {
        geminiModelSelect.value = savedModel;
      } else if (models.includes("gemini-1.5-pro")) {
        geminiModelSelect.value = "gemini-1.5-pro";
      } else if (models.includes("gemini-pro")) {
        geminiModelSelect.value = "gemini-pro";
      }
    } else {
      geminiModelSelect.innerHTML =
        '<option value="gemini-pro">Failed to load models (using default)</option>';
    }
  } catch (error) {
    console.error("Failed to load Gemini models:", error);
    geminiModelSelect.innerHTML =
      '<option value="gemini-pro">Error loading models (using default)</option>';
  } finally {
    geminiModelSelect.disabled = false;
  }
}

// Handler for Refresh models button
const refreshGeminiBtn = document.getElementById("refresh-gemini-models");
if (refreshGeminiBtn) {
  refreshGeminiBtn.addEventListener("click", (e) => {
    e.preventDefault();
    loadGeminiModels();
  });
}

// Model card selection logic
document.querySelectorAll(".model-card").forEach((card) => {
  card.addEventListener("click", () => {
    const radio = card.querySelector('input[type="radio"]');
    if (radio) {
      radio.checked = true;
      document
        .querySelectorAll(".model-card")
        .forEach((c) => c.classList.remove("active"));
      card.classList.add("active");
    }
  });
});

async function loadSystemInfo() {
  const systemInfoDiv = document.getElementById("system-info");
  if (!systemInfoDiv) return;

  try {
    const response = await window.electronAPI.apiRequest(
      "GET",
      "/api/system-info",
    );
    if (response.success) {
      const info = response.data;
      systemInfoDiv.innerHTML = `
        <div class="system-stats-grid">
            <div class="stat-item">
                <span class="stat-label">RAM Usage</span>
                <span class="stat-value">${info.total_ram_mb} MB Total</span>
                <span class="stat-sub">${info.available_ram_mb} MB Available</span>
            </div>
            <div class="stat-item">
                <span class="stat-label">CPU Cores</span>
                <span class="stat-value">${info.cpu_cores} Cores</span>
                <span class="stat-sub">Active System</span>
            </div>
            <div class="stat-item">
                <span class="stat-label">Current Mode</span>
                <span class="stat-value">${info.current_mode || "Auto"}</span>
                <span class="stat-sub">Optimization</span>
            </div>
            <div class="stat-item">
                <span class="stat-label">Storage</span>
                <span class="stat-value">Active</span>
                <span class="stat-sub">Index Ready</span>
            </div>
        </div>
      `;
    }
  } catch (error) {
    systemInfoDiv.innerHTML = "<p>Failed to load system information.</p>";
  }
}

async function loadLibrariesTable() {
  const tableBody = document.getElementById("libraries-table-body");
  if (!tableBody) return;

  try {
    const response = await window.electronAPI.apiRequest(
      "GET",
      "/api/settings",
    );
    if (response.success && response.data.indexed_directories) {
      const dirs = response.data.indexed_directories;

      if (dirs.length === 0) {
        tableBody.innerHTML =
          '<tr><td colspan="4" class="table-empty">No folders indexed yet. Add a folder to enable semantic search.</td></tr>';
      } else {
        const filesResponse = await window.electronAPI.apiRequest(
          "GET",
          "/api/files",
        );
        const fileCounts = {};

        if (filesResponse.success && filesResponse.data.files) {
          filesResponse.data.files.forEach((file) => {
            const dir = dirs.find((d) => file.file_path.startsWith(d));
            if (dir) {
              fileCounts[dir] = (fileCounts[dir] || 0) + 1;
            }
          });
        }

        tableBody.innerHTML = dirs
          .map(
            (dir) => `
          <tr>
            <td style="max-width:300px; overflow:hidden; text-overflow:ellipsis;" title="${escapeHtml(dir)}">${escapeHtml(dir)}</td>
            <td>${fileCounts[dir] || 0} files</td>
            <td>Recently</td>
            <td><span class="status-badge synced">Synced</span></td>
            <td style="text-align:right;">
              <button class="btn-secondary" style="padding: 0.25rem 0.75rem; font-size: 0.75rem;" onclick="removeDirectory('${escapeHtml(dir)}')">Remove</button>
            </td>
          </tr>
        `,
          )
          .join("");
      }
    }
  } catch (error) {
    tableBody.innerHTML =
      '<tr><td colspan="5" style="text-align: center; padding: 2rem;">Error loading libraries.</td></tr>';
  }
}

async function removeDirectory(path) {
  try {
    const response = await window.electronAPI.apiRequest(
      "GET",
      "/api/settings",
    );
    if (response.success) {
      const dirs = response.data.indexed_directories.filter((d) => d !== path);
      await window.electronAPI.apiRequest("PUT", "/api/settings", {
        indexed_directories: dirs,
      });
      loadLibrariesTable();
    }
  } catch (error) {
    console.error("Failed to remove directory:", error);
  }
}

window.removeDirectory = removeDirectory;

// Max results slider handler
const maxResultsSlider = document.getElementById("max-results-slider");
const maxResultsValue = document.getElementById("max-results-value");
if (maxResultsSlider && maxResultsValue) {
  maxResultsSlider.addEventListener("input", (e) => {
    const value = parseInt(e.target.value);
    maxResultsValue.textContent = value;
    maxSearchResults = value;
  });
}

// Set up AI settings UI handlers
const aiEnabledCheckbox = document.getElementById("ai-features-enabled");
if (aiEnabledCheckbox) {
  aiEnabledCheckbox.addEventListener("change", (e) => {
    const aiSettingsContainer = document.getElementById(
      "ai-settings-container",
    );
    if (aiSettingsContainer) {
      aiSettingsContainer.style.display = e.target.checked ? "block" : "none";
    }
  });
}

const aiProviderSelect = document.getElementById("ai-provider-select");
if (aiProviderSelect) {
  aiProviderSelect.addEventListener("change", (e) => {
    updateAiProviderUI(e.target.value);
  });
}

const saveSettingsBtn = document.getElementById("save-settings");
if (saveSettingsBtn) {
  saveSettingsBtn.addEventListener("click", async () => {
    const checkedMode = document.querySelector(
      'input[name="performance-mode"]:checked',
    );
    if (!checkedMode) return;

    const selectedMode = checkedMode.value;
    const messageDiv = document.getElementById("settings-message");
    const saveBtn = document.getElementById("save-settings");

    // Get max search results from slider
    const maxResults = maxResultsSlider
      ? parseInt(maxResultsSlider.value)
      : maxSearchResults;

    // Get AI settings
    const aiEnabled = aiEnabledCheckbox ? aiEnabledCheckbox.checked : false;
    const aiProvider = aiProviderSelect ? aiProviderSelect.value : "ollama";

    // Read current values from the DOM (these may not be in outer scope)
    const ollamaModelInputEl =
      document.getElementById("ollama-model-input");
    const geminiModelSelectEl =
      document.getElementById("gemini-model-select");
    const apiKeyInputEl = document.getElementById("api-key-input");
    const parsingModelSelectEl = document.getElementById(
      "action-search-parsing-model",
    );
    const analysisModelSelectEl = document.getElementById(
      "action-search-analysis-model",
    );

    const ollamaModel = ollamaModelInputEl
      ? ollamaModelInputEl.value
      : null;
    const geminiModel = geminiModelSelectEl
      ? geminiModelSelectEl.value
      : null;
    const apiKey = apiKeyInputEl ? apiKeyInputEl.value.trim() : null;

    // Get Action Search settings (provider overrides)
    const parsingModel = parsingModelSelectEl
      ? parsingModelSelectEl.value
      : "ollama";
    const analysisModel = analysisModelSelectEl
      ? analysisModelSelectEl.value
      : "same-as-main";

    // Get filter duplicate files
    const filterDuplicateCheckbox = document.getElementById(
      "filter-duplicate-files",
    );
    const filterDuplicateFiles = filterDuplicateCheckbox
      ? filterDuplicateCheckbox.checked
      : true;

    // Get excluded extensions
    const excludedInput = document.getElementById("excluded-extensions-input");
    const excludedExtensions = excludedInput
      ? excludedInput.value
          .split(",")
          .map((ext) => ext.trim().toLowerCase())
          .filter((ext) => ext.length > 0)
      : [];

    // Disable save button and show loading state
    saveSettingsBtn.disabled = true;
    saveSettingsBtn.textContent = "Saving...";
    if (messageDiv) {
      messageDiv.textContent = "";
      messageDiv.className = "settings-message";
    }

    console.log("[FRONTEND] Saving settings:", {
      performance_mode: selectedMode,
      max_search_results: maxResults,
      ai_features_enabled: aiEnabled,
      ai_provider: aiProvider,
      has_ollama_model: !!ollamaModel,
      has_gemini_model: !!geminiModel,
      has_api_key: !!apiKey,
      action_search_parsing_model: parsingModel,
      action_search_analysis_model: analysisModel,
      excluded_extensions_count: excludedExtensions.length,
    });

    try {
      const requestData = {
        performance_mode: selectedMode,
        max_search_results: maxResults,
        filter_duplicate_files: filterDuplicateFiles,
        ai_features_enabled: aiEnabled,
        ai_provider: aiProvider,
      };

      if (ollamaModel) {
        requestData.ollama_model = ollamaModel;
      }

      if (geminiModel) {
        requestData.gemini_model = geminiModel;
      }

      if (apiKey) {
        requestData.api_key = apiKey;
      }

      // Add Action Search settings
      requestData.action_search_parsing_model = parsingModel;
      requestData.action_search_analysis_model = analysisModel;

      // Add excluded extensions if any
      if (excludedExtensions.length > 0) {
        if (!requestData.file_type_filters) {
          requestData.file_type_filters = {};
        }
        requestData.file_type_filters.excluded_extensions = excludedExtensions;
      }

      console.log(
        "[FRONTEND] Sending request data:",
        JSON.stringify(requestData, null, 2),
      );

      const response = await window.electronAPI.apiRequest(
        "PUT",
        "/api/settings",
        requestData,
      );

      console.log(
        "[FRONTEND] Received response:",
        JSON.stringify(response, null, 2),
      );

      if (response.success) {
        // Show success message
        if (messageDiv) {
          messageDiv.textContent = "✓ Settings saved successfully!";
          messageDiv.className = "settings-message success";
        }

        console.log("[FRONTEND] Settings saved successfully!");

        // Reload settings to verify they were saved correctly
        setTimeout(async () => {
          console.log("[FRONTEND] Reloading settings to verify...");
          const verifySettings = await window.electronAPI.apiRequest(
            "GET",
            "/api/settings",
          );
          console.log("[FRONTEND] Verified settings:", {
            ai_features_enabled: verifySettings.ai_features_enabled,
            ai_provider: verifySettings.ai_provider,
            performance_mode: verifySettings.performance_mode,
          });

          // Update UI with verified settings
          if (
            aiEnabledCheckbox &&
            verifySettings.ai_features_enabled !== undefined
          ) {
            const verifiedEnabled = verifySettings.ai_features_enabled === true;
            console.log("[FRONTEND] Setting checkbox to:", verifiedEnabled);
            aiEnabledCheckbox.checked = verifiedEnabled;
            if (aiSettingsContainer) {
              aiSettingsContainer.style.display = verifiedEnabled
                ? "block"
                : "none";
            }
          }
        }, 500);
      } else {
        console.error("[FRONTEND] Save failed - response.success was false");
        if (messageDiv) {
          messageDiv.textContent = "Failed to save settings";
          messageDiv.className = "settings-message error";
        }
      }
    } catch (error) {
      console.error("[FRONTEND] Error saving settings:", error);
      if (messageDiv) {
        messageDiv.textContent = `Error: ${error?.message || error?.toString() || "Unknown error"}`;
        messageDiv.className = "settings-message error";
      }
    } finally {
      // Re-enable save button
      saveSettingsBtn.disabled = false;
      saveSettingsBtn.textContent = "Save Settings";
    }
  });
}

const clearIndexBtn = document.getElementById("clear-index");
if (clearIndexBtn) {
  clearIndexBtn.addEventListener("click", async () => {
    const confirmClear = confirm(
      "Purge Index? This deletes all embeddings but keeps folders in the list. Your files remain untouched. Search will not work until re-indexing.",
    );
    if (!confirmClear) return;

    try {
      // Clear the index (files and embeddings) but keep folders in settings
      const response = await window.electronAPI.apiRequest(
        "POST",
        "/api/index/clear",
      );
      if (response.success) {
        showSuccess(
          'Index purged. You can re-index folders by clicking "Add Folder" again.',
        );
        loadLibrariesTable();
      }
    } catch (error) {
      showError("Purge failed: " + error.message);
    }
  });
}

const addDirectoryBtn = document.getElementById("add-directory");
if (addDirectoryBtn) {
  addDirectoryBtn.addEventListener("click", async () => {
    try {
      const result = await window.electronAPI.selectDirectory();
      if (result.success && result.path) {
        const settingsResponse = await window.electronAPI.apiRequest(
          "GET",
          "/api/settings",
        );
        if (settingsResponse.success) {
          const currentDirs = settingsResponse.data.indexed_directories || [];
          let needsUpdate = false;
          let updatedDirs = currentDirs;

          // If directory not in list, add it
          if (!currentDirs.includes(result.path)) {
            updatedDirs = [...currentDirs, result.path];
            needsUpdate = true;
          }

          // Update settings if needed
          if (needsUpdate) {
            const response = await window.electronAPI.apiRequest(
              "PUT",
              "/api/settings",
              {
                indexed_directories: updatedDirs,
              },
            );
            if (!response.success) {
              showError(
                "Failed to update settings: " +
                  (response.error || "Unknown error"),
              );
              return;
            }
          }

          // Always start indexing (even if already in list, allows re-indexing)
          await window.electronAPI.apiRequest("POST", "/api/index/start", {
            directory: result.path,
          });
          loadLibrariesTable();
          // Start polling for indexing progress
          startIndexingProgressPoll();
        }
      }
    } catch (error) {
      console.error("Failed to add directory:", error);
      showError("Failed to add directory: " + error.message);
    }
  });
}

// Backend Health
async function checkBackendConnection() {
  try {
    const response = await window.electronAPI.apiRequest("GET", "/api/health");
    if (response.success) {
      updateSidebarStatus("System Ready", 100);
    } else {
      updateSidebarStatus("Backend Offline", 0);
    }
  } catch (error) {
    updateSidebarStatus("Connecting...", 0);
  }
}

function updateSidebarStatus(text, progress) {
  const statusText = document.getElementById("sidebar-progress-text");
  const statusFill = document.getElementById("sidebar-progress");
  if (statusText) statusText.textContent = text;
  if (statusFill) statusFill.style.width = `${progress}%`;
}

let indexingProgressInterval = null;

async function checkIndexingProgress() {
  try {
    const response = await window.electronAPI.apiRequest(
      "GET",
      "/api/index/status",
    );
    if (response.success && response.data) {
      const status = response.data;
      if (
        status.is_indexing &&
        status.current !== null &&
        status.total !== null
      ) {
        const percent =
          status.total > 0
            ? Math.round((status.current / status.total) * 100)
            : 0;
        const fileName = status.current_file
          ? status.current_file.split(/[\\/]/).pop()
          : "";
        updateSidebarStatus(
          `Indexing: ${status.current}/${status.total} ${fileName ? `(${fileName})` : ""}`,
          percent,
        );
        return true; // Still indexing
      } else {
        // Not indexing, check backend health
        await checkBackendConnection();
        return false; // Not indexing
      }
    }
  } catch (error) {
    // If error, fall back to health check
    await checkBackendConnection();
  }
  return false;
}

function startIndexingProgressPoll() {
  // Clear any existing interval
  if (indexingProgressInterval) {
    clearInterval(indexingProgressInterval);
  }

  // Poll every 500ms while indexing
  indexingProgressInterval = setInterval(async () => {
    const isIndexing = await checkIndexingProgress();
    if (!isIndexing && indexingProgressInterval) {
      clearInterval(indexingProgressInterval);
      indexingProgressInterval = null;
    }
  }, 500);
}

// File Browser functionality
let currentBrowserPath = "";
let selectedBrowserItem = null;
let specialFolders = {};

// Sort settings per page
let sortSettings = {
  desktop: { sort: "name", order: "asc" },
  downloads: { sort: "name", order: "asc" },
  documents: { sort: "name", order: "asc" },
};

// Load special folders on startup
async function loadSpecialFolders() {
  try {
    const response = await window.electronAPI.apiRequest(
      "GET",
      "/api/files/special-folders",
    );
    if (response.success && response.data) {
      specialFolders = response.data;
    }
  } catch (error) {
    console.error("Failed to load special folders:", error);
  }
}

// Tree view state
let treeViewVisible = false;
let currentTreeRoot = null;

// Load and render tree view
async function loadTreeView(rootPath = null) {
  const treeContainer = document.getElementById("tree-view-container");
  if (!treeContainer) return;

  try {
    const path = rootPath || specialFolders.home || "::this-pc";
    const url = `/api/files/tree?path=${encodeURIComponent(path)}&depth=2`;
    const response = await window.electronAPI.apiRequest("GET", url);

    if (response.success && response.data && response.data.nodes) {
      currentTreeRoot = path;
      renderTreeNodes(response.data.nodes, treeContainer, 0);
    }
  } catch (error) {
    console.error("Failed to load tree view:", error);
  }
}

// Render tree nodes recursively
function renderTreeNodes(nodes, container, depth) {
  // Clear container if depth is 0 (root level)
  if (depth === 0) {
    container.innerHTML = "";
  }

  nodes.forEach((node) => {
    const nodeElement = createTreeNodeElement(node, depth);
    container.appendChild(nodeElement);
  });
}

// Create a tree node element
function createTreeNodeElement(node, depth) {
  const nodeDiv = document.createElement("div");
  nodeDiv.className = "tree-node";
  nodeDiv.dataset.path = node.path;
  nodeDiv.dataset.isDirectory = node.is_directory;

  const itemDiv = document.createElement("div");
  itemDiv.className = "tree-node-item";
  itemDiv.style.paddingLeft = `${depth * 1}rem`;

  // Expand/collapse button (only for directories)
  const expandBtn = document.createElement("div");
  expandBtn.className = "tree-node-expand";
  if (node.is_directory) {
    if (node.children && node.children.length > 0) {
      expandBtn.textContent = "▶";
      expandBtn.classList.toggle("expanded", node.expanded);
    } else if (node.children === null) {
      // Not loaded yet - show placeholder
      expandBtn.textContent = "▶";
    } else {
      // Empty directory
      expandBtn.textContent = "";
    }
  } else {
    expandBtn.textContent = "";
  }

  // Icon
  const iconDiv = document.createElement("div");
  iconDiv.className = "tree-node-icon";
  if (node.is_directory) {
    iconDiv.innerHTML =
      '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"></path></svg>';
  } else {
    const iconData = getFileIcon(node.name);
    iconDiv.innerHTML = iconData.icon;
  }

  // Name
  const nameDiv = document.createElement("div");
  nameDiv.className = "tree-node-name";
  nameDiv.textContent = node.name;
  nameDiv.title = node.path;

  itemDiv.appendChild(expandBtn);
  itemDiv.appendChild(iconDiv);
  itemDiv.appendChild(nameDiv);

  // Children container
  const childrenDiv = document.createElement("div");
  childrenDiv.className = "tree-node-children";
  if (node.expanded && node.children) {
    childrenDiv.classList.add("expanded");
    node.children.forEach((child) => {
      childrenDiv.appendChild(createTreeNodeElement(child, depth + 1));
    });
  }

  // Click handler
  itemDiv.addEventListener("click", async (e) => {
    e.stopPropagation();

    if (node.is_directory) {
      // Toggle expand/collapse
      if (node.children === null) {
        // Lazy load children
        try {
          const url = `/api/files/tree?path=${encodeURIComponent(node.path)}&depth=2`;
          const response = await window.electronAPI.apiRequest("GET", url);
          if (response.success && response.data && response.data.nodes) {
            node.children = response.data.nodes;
            node.expanded = true;
            // Re-render this node's children
            renderTreeNodes(node.children, childrenDiv, depth + 1);
            childrenDiv.classList.add("expanded");
            expandBtn.classList.add("expanded");
          }
        } catch (error) {
          console.error("Failed to load tree children:", error);
        }
      } else {
        // Toggle existing children
        node.expanded = !node.expanded;
        childrenDiv.classList.toggle("expanded", node.expanded);
        expandBtn.classList.toggle("expanded", node.expanded);
      }
    } else {
      // File clicked - open preview
      await openPreviewPanel(node.path);
    }
  });

  // Double click to navigate (for directories)
  if (node.is_directory) {
    itemDiv.addEventListener("dblclick", async (e) => {
      e.stopPropagation();
      // Navigate to this folder in the current page
      const activePage = document.querySelector(".page.active");
      if (activePage) {
        const pageId = activePage.id;
        if (pageId.includes("-page")) {
          const pageType = pageId.replace("-page", "");
          await loadFolderFiles(node.path, `${pageType}-file-list`, pageType);
        }
      }
    });
  }

  nodeDiv.appendChild(itemDiv);
  nodeDiv.appendChild(childrenDiv);

  return nodeDiv;
}

// Toggle tree view visibility
function toggleTreeView() {
  const treeSidebar = document.getElementById("tree-view-sidebar");
  const treeShowBtn = document.getElementById("tree-view-show-btn");

  if (!treeSidebar) return;

  treeViewVisible = !treeViewVisible;

  if (treeViewVisible) {
    treeSidebar.style.display = "flex";
    if (treeShowBtn) treeShowBtn.style.display = "none";
    // Load tree if not loaded
    if (!currentTreeRoot) {
      loadTreeView();
    }
  } else {
    treeSidebar.style.display = "none";
    if (treeShowBtn) treeShowBtn.style.display = "flex";
  }
}

// Load folder page based on page type
async function loadFolderPage(pageType) {
  let folderPath = null;
  let fileListId = null;

  switch (pageType) {
    case "desktop":
      folderPath = specialFolders.desktop;
      fileListId = "desktop-file-list";
      break;
    case "downloads":
      folderPath = specialFolders.downloads;
      fileListId = "downloads-file-list";
      break;
    case "documents":
      folderPath = specialFolders.documents;
      fileListId = "documents-file-list";
      break;
    case "other-files":
      folderPath = "::this-pc"; // Show "This PC" (drives/root directories)
      fileListId = "other-files-file-list";
      break;
  }

  if (!folderPath) {
    // Load special folders if not already loaded
    await loadSpecialFolders();
    switch (pageType) {
      case "desktop":
        folderPath = specialFolders.desktop;
        break;
      case "downloads":
        folderPath = specialFolders.downloads;
        break;
      case "documents":
        folderPath = specialFolders.documents;
        break;
      case "other-files":
        folderPath = "::this-pc"; // Always show "This PC" (drives/root directories)
        break;
    }
  }

  if (folderPath && fileListId) {
    // Check if there's a search query for this page
    const searchInput = document.getElementById(`file-search-${pageType}`);
    const searchQuery = searchInput ? searchInput.value.trim() : null;
    await loadFolderFiles(folderPath, fileListId, pageType, searchQuery);
  }
}

// Load files for a specific folder and display them
async function loadFolderFiles(
  folderPath,
  fileListId,
  pageType = null,
  searchQuery = null,
) {
  try {
    let response;

    if (searchQuery && searchQuery.trim()) {
      // Use search endpoint
      const url = `/api/files/search?query=${encodeURIComponent(searchQuery)}&path=${encodeURIComponent(folderPath)}&limit=100`;
      response = await window.electronAPI.apiRequest("GET", url);
      if (response.success && response.data) {
        displayFolderFiles(response.data.results, fileListId);
      } else {
        showToast(
          "Search failed: " + (response.error || "Unknown error"),
          "error",
        );
      }
    } else {
      // Use browse endpoint with sorting
      let sort = "name";
      let order = "asc";
      if (pageType && sortSettings[pageType]) {
        sort = sortSettings[pageType].sort;
        order = sortSettings[pageType].order;
      }

      const url = `/api/files/browse?path=${encodeURIComponent(folderPath)}&sort=${sort}&order=${order}`;
      response = await window.electronAPI.apiRequest("GET", url);
      if (response.success && response.data) {
        displayFolderFiles(response.data.items, fileListId);
      } else {
        showToast(
          "Failed to load folder: " + (response.error || "Unknown error"),
          "error",
        );
      }
    }
  } catch (error) {
    console.error("Failed to load folder:", error);
    showToast("Failed to load folder: " + error.message, "error");
  }
}

// Display files in a folder list
function displayFolderFiles(items, fileListId) {
  const fileList = document.getElementById(fileListId);
  if (!fileList) return;

  if (items.length === 0) {
    fileList.innerHTML =
      '<div class="empty-state"><h3>Folder is empty</h3></div>';
    return;
  }

  // Don't sort here - backend already sorted the items according to user's sort preferences

  // Determine if list view
  const isListView = viewPreferences.view === "list";

  fileList.innerHTML = items
    .map((item) => {
      const iconData = getFileIcon(item.name, item.is_directory);
      const size = item.size ? formatFileSize(item.size) : "";
      const date = item.modified_time
        ? new Date(item.modified_time * 1000).toLocaleDateString()
        : "";

      if (isListView) {
        // Compact list view layout
        return `
                <div class="file-item" data-path="${escapeHtml(item.path)}" data-is-dir="${item.is_directory}">
                    <div class="file-icon-wrapper" data-file-type="${iconData.category}">${iconData.icon}</div>
                    <div class="file-item-name" title="${escapeHtml(item.name)}">${escapeHtml(item.name)}</div>
                    ${size ? `<div class="file-item-details">${size}</div>` : ""}
                    ${date ? `<div class="file-item-details">${date}</div>` : ""}
                </div>
            `;
      } else {
        // Grid view layout
        return `
                <div class="file-item" data-path="${escapeHtml(item.path)}" data-is-dir="${item.is_directory}">
                    <div class="file-icon-wrapper" data-file-type="${iconData.category}">${iconData.icon}</div>
                    <div class="file-item-name" title="${escapeHtml(item.name)}">${escapeHtml(item.name)}</div>
                    ${size ? `<div class="file-item-details">${size}</div>` : ""}
                    ${date ? `<div class="file-item-details">${date}</div>` : ""}
                </div>
            `;
      }
    })
    .join("");

  // Apply view preferences to the list
  applyViewToElement(fileList);

  // Add click handlers
  fileList.querySelectorAll(".file-item").forEach((item) => {
    let clickTimer = null;

    item.addEventListener("click", async (e) => {
      const filePath = item.dataset.path;
      const isDir = item.dataset.isDir === "true";

      // If timer exists, this may be the second click of a double-click - clear and wait for dblclick
      if (clickTimer) {
        clearTimeout(clickTimer);
        clickTimer = null;
        return;
      }

      if (isDir) {
        // For directories, navigation happens on double click
        return;
      }

      // Single click on file: open preview panel (delay to distinguish from double click)
      clickTimer = setTimeout(async () => {
        clickTimer = null;
        await openPreviewPanel(filePath);
      }, 150);
    });

    item.addEventListener("dblclick", async (e) => {
      // Clear single click timer
      if (clickTimer) {
        clearTimeout(clickTimer);
        clickTimer = null;
      }

      const filePath = item.dataset.path;
      const isDir = item.dataset.isDir === "true";

      if (isDir) {
        // Double click on folder: navigate into folder
        // Get current folder path from the page context
        const currentPage = document.querySelector(".page.active");
        let currentFolderPath = null;

        if (currentPage) {
          const pageId = currentPage.id;
          if (pageId === "desktop-page" && specialFolders.desktop) {
            currentFolderPath = specialFolders.desktop;
          } else if (pageId === "downloads-page" && specialFolders.downloads) {
            currentFolderPath = specialFolders.downloads;
          } else if (pageId === "documents-page" && specialFolders.documents) {
            currentFolderPath = specialFolders.documents;
          } else if (pageId === "other-files-page" && specialFolders.home) {
            currentFolderPath = specialFolders.home;
          }
        }

        // Navigate to the folder (pass pageType for sort settings)
        const pageType = fileListId.replace("-file-list", "");
        await loadFolderFiles(filePath, fileListId, pageType);

        // Update breadcrumb
        const currentFolder = document.getElementById("current-folder");
        if (currentFolder) {
          const folderName = filePath.split(/[\\/]/).pop();
          currentFolder.textContent = folderName;
        }
      } else {
        // Double click on file: open externally
        await openFile(filePath);
      }
    });
  });
}

// Apply view preferences to a specific element
function applyViewToElement(element) {
  if (!element) return;

  // Apply view type
  if (viewPreferences.view === "list") {
    element.classList.add("list-view");
    element.classList.remove("grid-view");
    element.style.gridTemplateColumns = "1fr";
  } else {
    element.classList.add("grid-view");
    element.classList.remove("list-view");
    const minWidths = {
      small: "120px",
      medium: "180px",
      large: "240px",
    };
    element.style.gridTemplateColumns = `repeat(auto-fill, minmax(${minWidths[viewPreferences.size]}, 1fr))`;
  }

  // Apply size
  element.classList.remove("size-small", "size-medium", "size-large");
  element.classList.add(`size-${viewPreferences.size}`);
}

// Browse directory
async function browseDirectory(path) {
  try {
    const response = await window.electronAPI.apiRequest(
      "GET",
      `/api/files/browse?path=${encodeURIComponent(path)}`,
    );
    if (response.success && response.data) {
      currentBrowserPath = response.data.path;
      const pathEl = document.getElementById("browser-path");
      if (pathEl) pathEl.textContent = response.data.path;
      displayBrowserItems(response.data.items);
    }
  } catch (error) {
    console.error("Failed to browse directory:", error);
    showError("Failed to browse directory: " + error.message);
  }
}

function displayBrowserItems(items) {
  const fileList = document.getElementById("browser-file-list");
  if (!fileList) return;

  if (items.length === 0) {
    fileList.innerHTML =
      '<div class="empty-state"><h3>Folder is empty</h3></div>';
    return;
  }

  fileList.innerHTML = items
    .map((item) => {
      const iconData = getFileIcon(item.name, item.is_directory);

      const size = item.size ? formatFileSize(item.size) : "";
      const date = item.modified_time
        ? new Date(item.modified_time * 1000).toLocaleDateString()
        : "";

      return `
            <div class="browser-file-item" data-path="${escapeHtml(item.path)}" data-is-dir="${item.is_directory}">
                <div class="browser-file-icon" data-file-type="${iconData.category}">${iconData.icon}</div>
                <div class="browser-file-name" title="${escapeHtml(item.name)}">${escapeHtml(item.name)}</div>
                ${size || date ? `<div class="browser-file-info">${size} ${date}</div>` : ""}
            </div>
        `;
    })
    .join("");

  // Add click handlers
  fileList.querySelectorAll(".browser-file-item").forEach((item) => {
    item.addEventListener("click", (e) => {
      // Remove previous selection
      fileList
        .querySelectorAll(".browser-file-item")
        .forEach((i) => i.classList.remove("selected"));
      item.classList.add("selected");
      selectedBrowserItem = {
        path: item.dataset.path,
        isDirectory: item.dataset.isDir === "true",
      };

      // Enable/disable toolbar buttons
      const deleteBtn = document.getElementById("browser-delete");
      const renameBtn = document.getElementById("browser-rename");
      const addIndexBtn = document.getElementById("browser-add-to-index");
      if (deleteBtn) deleteBtn.disabled = false;
      if (renameBtn) renameBtn.disabled = false;
      if (addIndexBtn) addIndexBtn.disabled = !selectedBrowserItem.isDirectory;

      // Double click to open directory
      if (e.detail === 2 && selectedBrowserItem.isDirectory) {
        browseDirectory(selectedBrowserItem.path);
      }
    });
  });
}

function formatFileSize(bytes) {
  if (bytes < 1024) return bytes + " B";
  if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + " KB";
  return (bytes / (1024 * 1024)).toFixed(1) + " MB";
}

// Quick access handlers
document.querySelectorAll(".quick-access-item").forEach((btn) => {
  btn.addEventListener("click", async () => {
    const folderType = btn.dataset.path;
    if (specialFolders[folderType]) {
      await browseDirectory(specialFolders[folderType]);
    } else {
      await loadSpecialFolders();
      if (specialFolders[folderType]) {
        await browseDirectory(specialFolders[folderType]);
      }
    }
  });
});

// Browser toolbar handlers
const browserBackBtn = document.getElementById("browser-back");
if (browserBackBtn) {
  browserBackBtn.addEventListener("click", () => {
    if (currentBrowserPath) {
      const parts = currentBrowserPath.split(/[\\/]/).filter((p) => p);
      if (parts.length > 1) {
        parts.pop();
        const parent =
          parts.join("/") ||
          (currentBrowserPath.includes("\\") ? parts.join("\\") : "/");
        browseDirectory(parent);
      }
    }
  });
}

const browserNewFolderBtn = document.getElementById("browser-new-folder");
if (browserNewFolderBtn) {
  browserNewFolderBtn.addEventListener("click", async () => {
    const name = prompt("Enter folder name:");
    if (!name) return;

    try {
      const response = await window.electronAPI.apiRequest(
        "POST",
        "/api/files/create-folder",
        {
          path: currentBrowserPath || specialFolders.home || "",
          name: name,
        },
      );
      if (response.success) {
        await browseDirectory(currentBrowserPath);
      } else {
        showError(
          "Failed to create folder: " + (response.error || "Unknown error"),
        );
      }
    } catch (error) {
      showError("Failed to create folder: " + error.message);
    }
  });
}

const browserDeleteBtn = document.getElementById("browser-delete");
if (browserDeleteBtn) {
  browserDeleteBtn.addEventListener("click", async () => {
    if (!selectedBrowserItem) return;

    const itemName = selectedBrowserItem.path.split(/[\\/]/).pop();
    const confirmMsg = selectedBrowserItem.isDirectory
      ? `Delete folder "${itemName}" and all its contents?`
      : `Delete file "${itemName}"?`;

    if (!confirm(confirmMsg)) return;

    try {
      const response = await window.electronAPI.apiRequest(
        "POST",
        "/api/files/delete",
        {
          path: selectedBrowserItem.path,
        },
      );
      if (response.success) {
        selectedBrowserItem = null;
        await browseDirectory(currentBrowserPath);
      } else {
        showError("Failed to delete: " + (response.error || "Unknown error"));
      }
    } catch (error) {
      showError("Failed to delete: " + error.message);
    }
  });
}

const browserRenameBtn = document.getElementById("browser-rename");
if (browserRenameBtn) {
  browserRenameBtn.addEventListener("click", async () => {
    if (!selectedBrowserItem) return;

    const oldName = selectedBrowserItem.path.split(/[\\/]/).pop();
    const newName = prompt("Enter new name:", oldName);
    if (!newName || newName === oldName) return;

    try {
      const response = await window.electronAPI.apiRequest(
        "PUT",
        "/api/files/rename",
        {
          path: selectedBrowserItem.path,
          new_name: newName,
        },
      );
      if (response.success) {
        selectedBrowserItem = null;
        await browseDirectory(currentBrowserPath);
      } else {
        showError("Failed to rename: " + (response.error || "Unknown error"));
      }
    } catch (error) {
      showError("Failed to rename: " + error.message);
    }
  });
}

const browserAddIndexBtn = document.getElementById("browser-add-to-index");
if (browserAddIndexBtn) {
  browserAddIndexBtn.addEventListener("click", async () => {
    if (!selectedBrowserItem || !selectedBrowserItem.isDirectory) return;

    try {
      // Add to indexed directories
      const settingsResponse = await window.electronAPI.apiRequest(
        "GET",
        "/api/settings",
      );
      if (settingsResponse.success) {
        const currentDirs = settingsResponse.data.indexed_directories || [];
        if (!currentDirs.includes(selectedBrowserItem.path)) {
          const response = await window.electronAPI.apiRequest(
            "PUT",
            "/api/settings",
            {
              indexed_directories: [...currentDirs, selectedBrowserItem.path],
            },
          );

          if (response.success) {
            await window.electronAPI.apiRequest("POST", "/api/index/start", {
              directory: selectedBrowserItem.path,
            });
            showSuccess("Folder added to index and indexing started!");
            startIndexingProgressPoll();
          }
        } else {
          showInfo("Folder is already indexed");
        }
      }
    } catch (error) {
      showError("Failed to add to index: " + error.message);
    }
  });
}

// Note: Folder page loading is handled in the main navigation handler above

// Initialize
checkBackendConnection();
setInterval(checkBackendConnection, 10000);
// Also check indexing progress periodically
setInterval(checkIndexingProgress, 2000);
loadSettings();
loadSystemInfo();
loadLibrariesTable();
loadSpecialFolders();
loadSearchHistory();
loadViewPreferences();
applyViewPreferences();

// Set up preview panel close button
const previewCloseBtn = document.getElementById("preview-close-btn");
if (previewCloseBtn) {
  previewCloseBtn.addEventListener("click", closePreviewPanel);
}

// Close preview panel when clicking outside (optional)
const previewPanel = document.getElementById("preview-panel");
if (previewPanel) {
  previewPanel.addEventListener("click", (e) => {
    if (e.target === previewPanel) {
      closePreviewPanel();
    }
  });
}

// Set up PDF navigation buttons
const pdfPrevBtn = document.getElementById("pdf-prev-page");
const pdfNextBtn = document.getElementById("pdf-next-page");

if (pdfPrevBtn) {
  pdfPrevBtn.addEventListener("click", async () => {
    if (currentPdfDoc && currentPdfPageNum > 1) {
      currentPdfPageNum--;
      const previewPdf = document.getElementById("preview-pdf");
      await renderPdfPage(currentPdfPageNum, previewPdf);
      updatePdfPageInfo();
    }
  });
}

if (pdfNextBtn) {
  pdfNextBtn.addEventListener("click", async () => {
    if (currentPdfDoc && currentPdfDoc.numPages > currentPdfPageNum) {
      currentPdfPageNum++;
      const previewPdf = document.getElementById("preview-pdf");
      await renderPdfPage(currentPdfPageNum, previewPdf);
      updatePdfPageInfo();
    }
  });
}

// Set up tree view toggle buttons
const treeViewToggle = document.getElementById("tree-view-toggle");
const treeViewShowBtn = document.getElementById("tree-view-show-btn");

if (treeViewToggle) {
  treeViewToggle.addEventListener("click", toggleTreeView);
}

if (treeViewShowBtn) {
  treeViewShowBtn.addEventListener("click", toggleTreeView);
}

// Set up sort controls and search for each page
["desktop", "downloads", "documents"].forEach((pageType) => {
  const sortSelect = document.getElementById(`sort-select-${pageType}`);
  const sortOrderBtn = document.getElementById(`sort-order-${pageType}`);
  const searchInput = document.getElementById(`file-search-${pageType}`);

  if (sortSelect) {
    sortSelect.addEventListener("change", async (e) => {
      sortSettings[pageType].sort = e.target.value;
      // Reload current folder with new sort
      const currentPage = document.querySelector(".page.active");
      if (currentPage && currentPage.id === `${pageType}-page`) {
        await loadFolderPage(pageType);
      }
    });
  }

  if (sortOrderBtn) {
    sortOrderBtn.addEventListener("click", async () => {
      const currentOrder = sortOrderBtn.dataset.order;
      const newOrder = currentOrder === "asc" ? "desc" : "asc";
      sortOrderBtn.dataset.order = newOrder;
      sortSettings[pageType].order = newOrder;

      // Update icon rotation
      const svg = sortOrderBtn.querySelector("svg");
      if (svg) {
        svg.style.transform =
          newOrder === "desc" ? "rotate(180deg)" : "rotate(0deg)";
      }

      // Reload current folder with new order
      const currentPage = document.querySelector(".page.active");
      if (currentPage && currentPage.id === `${pageType}-page`) {
        await loadFolderPage(pageType);
      }
    });
  }

  if (searchInput) {
    let searchTimeout = null;
    searchInput.addEventListener("input", (e) => {
      // Debounce search
      if (searchTimeout) {
        clearTimeout(searchTimeout);
      }
      searchTimeout = setTimeout(async () => {
        const currentPage = document.querySelector(".page.active");
        if (currentPage && currentPage.id === `${pageType}-page`) {
          await loadFolderPage(pageType);
        }
      }, 300);
    });

    searchInput.addEventListener("keypress", (e) => {
      if (e.key === "Enter") {
        if (searchTimeout) {
          clearTimeout(searchTimeout);
        }
        const currentPage = document.querySelector(".page.active");
        if (currentPage && currentPage.id === `${pageType}-page`) {
          loadFolderPage(pageType);
        }
      }
    });
  }
});

// Active RAG Functionality
let actionSearchEnabled = false;
let currentSearchResults = [];
let currentActiveRagResponse = null;
let isActiveRagSearching = false; // Guard to prevent duplicate requests

// Action Search Toggle
const actionSearchToggle = document.getElementById("action-search-toggle");
if (actionSearchToggle) {
  actionSearchToggle.addEventListener("change", async (e) => {
    const isChecked = e.target.checked;

    // If turning ON, show warning only the first time
    if (isChecked) {
      const warningAlreadyShown = localStorage.getItem("activeRagWarningShown") === "true";
      if (!warningAlreadyShown) {
        const confirmed = await showConfirmDialog(
          "Enable Action Search?",
          "Action Search uses AI to analyze your files. This process contributes to API credit usage and performs deep analysis of the documents found. Continue?",
          "Enable AI Search",
          "Cancel",
        );

        if (!confirmed) {
          e.target.checked = false;
          actionSearchEnabled = false;
          saveActionSearchState();
          return;
        }

        localStorage.setItem("activeRagWarningShown", "true");
      }
    }

    actionSearchEnabled = e.target.checked;
    console.log("Action Search enabled:", actionSearchEnabled);

    // Hide/show response bar and tabs based on toggle
    updateActiveRagVisibility();
    saveActionSearchState();

    if (actionSearchEnabled) {
      showToast("Action Search enabled", "success");
    }
  });
}

async function showConfirmDialog(
  title,
  message,
  confirmText = "Confirm",
  cancelText = "Cancel",
) {
  // Remove existing dialog if any
  const existing = document.querySelector(".action-search-modal");
  if (existing) existing.remove();

  return new Promise((resolve) => {
    const modal = document.createElement("div");
    modal.className = "action-search-modal confirm-modal";
    modal.innerHTML = `
      <div class="modal-overlay"></div>
      <div class="modal-content" style="max-width: 450px;">
        <div class="modal-header">
          <h3>${title}</h3>
          <button class="modal-close" id="confirm-modal-close">&times;</button>
        </div>
        <div class="modal-body">
          <p>${message}</p>
        </div>
        <div class="form-actions" style="margin-top: 1.5rem;">
          <button class="btn-secondary" id="confirm-modal-cancel">${cancelText}</button>
          <button class="btn-primary" id="confirm-modal-ok">${confirmText}</button>
        </div>
      </div>
    `;

    document.body.appendChild(modal);

    const closeBtn = modal.querySelector("#confirm-modal-close");
    const cancelBtn = modal.querySelector("#confirm-modal-cancel");
    const okBtn = modal.querySelector("#confirm-modal-ok");
    const overlay = modal.querySelector(".modal-overlay");

    const cleanup = (result) => {
      modal.remove();
      resolve(result);
    };

    closeBtn.addEventListener("click", () => cleanup(false));
    cancelBtn.addEventListener("click", () => cleanup(false));
    okBtn.addEventListener("click", () => cleanup(true));
    overlay.addEventListener("click", () => cleanup(false));
  });
}

// showActionSearchDialog removed - using main search query for Intent Decomposition

async function performActiveRagSearch(query, userQuestion) {
  if (!actionSearchEnabled) {
    showToast("Please enable Action Search to use AI analysis", "error");
    return;
  }

  // Prevent duplicate requests
  if (isActiveRagSearching) {
    console.log("[Active RAG] Search already in progress, skipping duplicate request");
    showToast("AI analysis already in progress, please wait...", "info");
    return;
  }

  isActiveRagSearching = true;
  console.log("[Active RAG] Starting search - query:", query, "question:", userQuestion);

  try {
    showLoadingState();

    const response = await window.electronAPI.apiRequest(
      "POST",
      "/api/search/active-rag",
      {
        query: query,
        user_question: userQuestion,
        document_limit: 3,
      },
    );

    console.log("[Active RAG] Response received:", response);

    if (response.success && response.data) {
      // Check backend-level success
      const activeRagData = response.data;
      if (activeRagData.success) {
        console.log("[Active RAG] Analysis successful, displaying results");
        currentActiveRagResponse = activeRagData;
        
        // Display the AI answer
        displayActiveRagResponse(activeRagData);
        
        // Convert sources to search results format and display in normal results area
        if (activeRagData.sources && activeRagData.sources.length > 0) {
          const vectorResults = activeRagData.sources.map(source => ({
            file_path: source.file_path,
            file_name: source.file_name,
            similarity: source.relevance_score,
            preview: source.excerpt || null
          }));
          
          // Store and display vector search results
          lastSearchResults = vectorResults;
          if (resultsCount) {
            resultsCount.textContent = `Found ${vectorResults.length} relevant documents`;
          }
          displayResults(vectorResults);
        }
      } else {
        console.error("[Active RAG] Analysis failed:", activeRagData.error);
        showToast(activeRagData.error || "AI analysis failed", "error");
        hideActiveRagComponents();
      }
    } else {
      console.error("[Active RAG] Request failed:", response);
      showToast(response.error || "Active RAG search failed", "error");
      hideActiveRagComponents();
    }
  } catch (error) {
    console.error("[Active RAG] Search error:", error);
    showToast("Failed to perform AI analysis: " + error.message, "error");
    hideActiveRagComponents();
  } finally {
    isActiveRagSearching = false;
    hideLoadingState();
    console.log("[Active RAG] Search completed, guard released");
  }
}

function hideActiveRagComponents() {
  const responseContainer = document.getElementById("active-rag-response");
  const tabsContainer = document.getElementById("document-tabs");
  if (responseContainer) {
    responseContainer.style.display = "none";
    // Also hide integrated tabs within the container
    const integratedTabs = responseContainer.querySelector(".document-tabs-integrated");
    if (integratedTabs) integratedTabs.style.display = "none";
  }
  if (tabsContainer) tabsContainer.style.display = "none";
}

function displayActiveRagResponse(response) {
  console.log("[Active RAG] displayActiveRagResponse called with:", response);
  
  const responseContainer = document.getElementById("active-rag-response");
  const responseContent = document.getElementById("response-content");
  const responseConfidence = document.getElementById("response-confidence");
  const sourcesList = document.getElementById("sources-list");

  if (!responseContainer) {
    console.error("[Active RAG] responseContainer not found!");
    return;
  }
  
  if (!responseContent) {
    console.error("[Active RAG] responseContent element not found!");
    return;
  }
  
  if (!responseConfidence) {
    console.error("[Active RAG] responseConfidence element not found!");
    return;
  }

  console.log("[Active RAG] All elements found, displaying response");
  responseContainer.style.display = "block";

  // Hide loading state if still visible
  const loadingState = responseContainer.querySelector(".loading-state");
  if (loadingState) {
    loadingState.style.display = "none";
  }

  // Display AI answer
  if (response.answer) {
    console.log("[Active RAG] Setting answer content, length:", response.answer.length);
    responseContent.innerHTML = `
            <div class="ai-answer">
                ${formatAnswerWithHighlights(response.answer)}
            </div>
        `;
    responseContent.style.display = "block";
  } else {
    console.warn("[Active RAG] No answer in response!");
    responseContent.innerHTML = `<div class="ai-answer">No answer provided.</div>`;
    responseContent.style.display = "block";
  }

  // Display confidence
  if (response.confidence) {
    responseConfidence.textContent = `${Math.round(response.confidence * 100)}% Confidence`;
    responseConfidence.style.display =
      response.confidence > 0.7 ? "block" : "none";
  } else {
    responseConfidence.style.display = "none";
  }

  // Display sources (compact, as chips)
  if (sourcesList && response.sources && response.sources.length > 0) {
    const usedSources = response.sources.filter((source) => source.used_in_answer);
    console.log("[Active RAG] Displaying", usedSources.length, "used sources");
    if (usedSources.length > 0) {
      sourcesList.innerHTML = usedSources
        .map(
          (source) => `
                <div class="source-chip">
                    <span class="source-chip-name">${escapeHtml(source.file_name)}</span>
                    <span class="source-chip-score">${Math.round(source.relevance_score * 100)}%</span>
                </div>
            `,
        )
        .join("");
      sourcesList.style.display = "flex";
    } else {
      sourcesList.innerHTML = "";
      sourcesList.style.display = "none";
    }
  } else {
    if (sourcesList) {
      sourcesList.innerHTML = "";
      sourcesList.style.display = "none";
    }
  }
  
  // Display document tabs inline if multiple sources
  const documentTabs = document.getElementById("document-tabs");
  if (documentTabs && response.sources && response.sources.length > 1) {
    displayDocumentTabsInline(response.sources, documentTabs);
  } else if (documentTabs) {
    documentTabs.style.display = "none";
  }
  
  console.log("[Active RAG] Response displayed successfully");
}

function displayDocumentTabsInline(sources, tabsContainer) {
  const tabsNavigation = document.getElementById("tabs-navigation");
  const tabsContent = document.getElementById("tabs-content");
  
  if (!tabsNavigation || !tabsContent) return;
  
  tabsContainer.style.display = "block";
  
  // Create tab buttons
  tabsNavigation.innerHTML = sources
    .map((source, index) => `
      <button class="tab-button-integrated ${index === 0 ? 'active' : ''}" 
              data-tab-index="${index}"
              onclick="switchDocumentTab(${index})">
        ${escapeHtml(source.file_name)}
      </button>
    `)
    .join("");
  
  // Display first document by default
  if (sources.length > 0) {
    switchDocumentTab(0);
  }
  
  // Store sources for tab switching
  window.activeRagSources = sources;
}

function switchDocumentTab(index) {
  const sources = window.activeRagSources || [];
  const tabsContent = document.getElementById("tabs-content");
  const tabButtons = document.querySelectorAll(".tab-button-integrated");
  
  if (!tabsContent || index >= sources.length) return;
  
  // Update active tab button
  tabButtons.forEach((btn, i) => {
    if (i === index) {
      btn.classList.add("active");
    } else {
      btn.classList.remove("active");
    }
  });
  
  // Display selected document
  const source = sources[index];
  tabsContent.innerHTML = `
    <div class="tab-document-card">
      <div class="tab-document-header">
        <div class="tab-document-name">${escapeHtml(source.file_name)}</div>
        <div class="tab-document-score">Relevance: ${Math.round(source.relevance_score * 100)}%</div>
      </div>
      ${source.excerpt ? `<div class="tab-document-excerpt">${escapeHtml(source.excerpt)}</div>` : ''}
      <a href="#" class="tab-document-link" onclick="openFile('${escapeHtml(source.file_path)}'); return false;">Open Full Document</a>
    </div>
  `;
}

// Helper function to escape HTML
function escapeHtml(text) {
  const div = document.createElement("div");
  div.textContent = text;
  return div.innerHTML;
}

function displayDocumentTabs(sources) {
  const tabsContainer = document.getElementById("document-tabs");
  const tabsNavigation = document.getElementById("tabs-navigation");
  const tabsContent = document.getElementById("tabs-content");

  if (!tabsContainer || !tabsNavigation || !tabsContent) return;

  // Only show tabs if there are sources
  if (sources && sources.length > 1) {
    tabsContainer.style.display = "block";

    // Create tab buttons
    const tabButtons = sources
      .map(
        (source, index) => `
            <button class="tab-button ${index === 0 ? "active" : ""}" 
                    onclick="switchToTab(${index})" 
                    title="View ${source.file_name}">
                ${source.file_name}
            </button>
        `,
      )
      .join("");

    tabsNavigation.innerHTML = tabButtons;

    // Create tab content panels
    const tabPanels = sources
      .map(
        (source, index) => `
            <div class="tab-content-panel ${index === 0 ? "active" : ""}" id="tab-${index}">
                <div class="tab-file-info">
                    <div class="tab-file-name">${source.file_name}</div>
                    <div class="tab-relevance">Relevance: ${Math.round(source.relevance_score * 100)}%</div>
                </div>
                ${
                  source.excerpt
                    ? `
                    <div class="tab-excerpt">${source.excerpt}</div>
                `
                    : ""
                }
                ${
                  source.key_contributions &&
                  source.key_contributions.length > 0
                    ? `
                    <div class="tab-contributions">
                        <div class="contribution-title">Key Contributions:</div>
                        <div class="contribution-list">
                            ${source.key_contributions
                              .map(
                                (contrib) =>
                                  `<span class="contribution-item">${contrib}</span>`,
                              )
                              .join("")}
                        </div>
                    </div>
                `
                    : ""
                }
                <div class="tab-file-link">
                    <a href="#" onclick="openFile('${source.file_path}')">Open Full Document</a>
                </div>
            </div>
        `,
      )
      .join("");

    tabsContent.innerHTML = tabPanels;
  } else {
    tabsContainer.style.display = "none";
  }
}

function switchToTab(tabIndex) {
  // Update tab buttons
  const tabButtons = document.querySelectorAll(".tab-button");
  tabButtons.forEach((btn, index) => {
    btn.classList.toggle("active", index === tabIndex);
  });

  // Update tab content panels
  const tabPanels = document.querySelectorAll(".tab-content-panel");
  tabPanels.forEach((panel, index) => {
    panel.classList.toggle("active", index === tabIndex);
  });
}

function updateActiveRagVisibility() {
  const responseContainer = document.getElementById("active-rag-response");
  const tabsContainer = document.getElementById("document-tabs");

  if (!actionSearchEnabled) {
    // Hide Active RAG components
    if (responseContainer) responseContainer.style.display = "none";
    if (tabsContainer) tabsContainer.style.display = "none";
  } else {
    // Show components if we have results
    if (responseContainer && currentActiveRagResponse) {
      responseContainer.style.display = "block";
    }
    if (
      tabsContainer &&
      currentActiveRagResponse &&
      currentActiveRagResponse.sources.length > 1
    ) {
      tabsContainer.style.display = "block";
    }
  }
}

function formatAnswerWithHighlights(answer) {
  let formatted = answer;
  
  // Remove common chatty phrases at the start
  formatted = formatted.replace(/^(I can provide|Here's|Let me|I'll|I will|I can|I would|I might)/gi, '');
  formatted = formatted.replace(/^(you with|you a|you the|you some)/gi, '');
  formatted = formatted.trim();
  
  // Format problem statements (Problem:, Question:, Given:, etc.)
  formatted = formatted.replace(/^(Problem|Question|Given|Find|Determine|Solve|Calculate):\s*/gmi, '<div class="problem-statement"><strong>$1:</strong></div>');
  
  // Format numbered lists (1. item, 2. item, etc.)
  formatted = formatted.replace(/^(\d+)\.\s+(.+)$/gm, '<div class="numbered-list-item"><span class="list-number">$1.</span> <span class="list-content">$2</span></div>');
  
  // Format multiple choice patterns (1. option 2. option 3. option)
  formatted = formatted.replace(/(\d+)\.\s+([^\d]+?)(?=\s+\d+\.|$)/g, '<div class="multiple-choice-option"><span class="choice-number">$1.</span> <span class="choice-text">$2</span></div>');
  
  // Format vectors [a, b, c] or [x, y]
  formatted = formatted.replace(/\[([^\]]+)\]/g, (match, content) => {
    // Check if it looks like a vector/matrix (contains numbers, commas, possibly spaces)
    if (/[\d\s,]+/.test(content)) {
      return `<span class="math-vector">[${content}]</span>`;
    }
    return match;
  });
  
  // Format equations (lines containing = with math-like content)
  formatted = formatted.replace(/^([^=\n]+=\s*[^\n]+)$/gm, (match) => {
    // Only format if it looks like an equation (has numbers, variables, operators)
    if (/[a-zA-Z]\s*=\s*[\d\w\s\+\-\*\/\(\)]+/.test(match)) {
      return `<div class="math-equation">${match}</div>`;
    }
    return match;
  });
  
  // Format inline code-like patterns (backticks or monospace patterns)
  formatted = formatted.replace(/`([^`]+)`/g, '<code class="inline-code">$1</code>');
  
  // Format bold patterns (**text** or *text*)
  formatted = formatted.replace(/\*\*([^\*]+)\*\*/g, '<strong>$1</strong>');
  formatted = formatted.replace(/\*([^\*]+)\*/g, '<em>$1</em>');
  
  // Basic highlighting for key terms
  formatted = formatted.replace(
    /\b(important|key|main|primary|significant)\b/gi,
    "<strong>$1</strong>",
  );
  
  // Format numbers and percentages
  formatted = formatted.replace(
    /\b(\d+[%]|USD|\$[\d,]+)/g,
    '<span class="highlight-number">$1</span>',
  );
  
  // Preserve line breaks
  formatted = formatted.replace(/\n\n/g, '</p><p>');
  formatted = formatted.replace(/\n/g, '<br>');
  
  // Wrap in paragraph if not already wrapped
  if (!formatted.startsWith('<div') && !formatted.startsWith('<p')) {
    formatted = `<p>${formatted}</p>`;
  }
  
  return formatted;
}

function showLoadingState() {
  const responseContainer = document.getElementById("active-rag-response");
  if (responseContainer) {
    // Don't replace the entire innerHTML - just show/hide loading overlay
    // Check if loading state already exists
    let loadingState = responseContainer.querySelector(".loading-state");
    if (!loadingState) {
      loadingState = document.createElement("div");
      loadingState.className = "loading-state";
      loadingState.innerHTML = `
                <div class="loading-spinner"></div>
                <div>Analyzing documents with AI...</div>
            `;
      responseContainer.appendChild(loadingState);
    }
    loadingState.style.display = "block";
    responseContainer.style.display = "block";
    
    // Hide the content elements while loading
    const responseContent = document.getElementById("response-content");
    const responseConfidence = document.getElementById("response-confidence");
    const sourcesList = document.getElementById("sources-list");
    if (responseContent) responseContent.style.display = "none";
    if (responseConfidence) responseConfidence.style.display = "none";
    if (sourcesList) sourcesList.style.display = "none";
  }
}

function hideLoadingState() {
  const responseContainer = document.getElementById("active-rag-response");
  if (responseContainer) {
    // Hide the loading state overlay
    const loadingState = responseContainer.querySelector(".loading-state");
    if (loadingState) {
      loadingState.style.display = "none";
    }
    
    // Show the content elements again
    const responseContent = document.getElementById("response-content");
    const responseConfidence = document.getElementById("response-confidence");
    const sourcesList = document.getElementById("sources-list");
    if (responseContent) responseContent.style.display = "block";
    if (responseConfidence) responseConfidence.style.display = "block";
    if (sourcesList) sourcesList.style.display = "block";
  }
}

// Initialize Action Search state - off by default, do not restore from localStorage
actionSearchEnabled = false;
if (actionSearchToggle) {
  actionSearchToggle.checked = false;
}

// Save Action Search state to localStorage
function saveActionSearchState() {
  localStorage.setItem("actionSearchEnabled", actionSearchEnabled.toString());
}
