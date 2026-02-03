// Window Controls
const minimizeBtn = document.querySelector('.window-control.minimize');
const maximizeBtn = document.querySelector('.window-control.maximize');
const closeBtn = document.querySelector('.window-control.close');

if (minimizeBtn) minimizeBtn.addEventListener('click', () => window.electronAPI.windowMinimize());
if (maximizeBtn) maximizeBtn.addEventListener('click', () => window.electronAPI.windowMaximize());
if (closeBtn) closeBtn.addEventListener('click', () => window.electronAPI.windowClose());

// Sidebar Navigation
document.querySelectorAll('.nav-item').forEach(item => {
  item.addEventListener('click', () => {
    const page = item.dataset.page;
    
    // Update active nav item
    document.querySelectorAll('.nav-item').forEach(n => n.classList.remove('active'));
    item.classList.add('active');
    
    // Show corresponding page
    document.querySelectorAll('.page').forEach(p => p.classList.remove('active'));
    const targetPage = document.getElementById(`${page}-page`);
    if (targetPage) {
        targetPage.classList.add('active');
    }
    
    // Update breadcrumb
    const currentFolder = document.getElementById('current-folder');
    if (currentFolder) {
        currentFolder.textContent = page.charAt(0).toUpperCase() + page.slice(1).replace('-', ' ');
    }
    
    // Load data when switching pages
    if (page === 'indexing') {
      loadSettings();
      loadSystemInfo();
      loadLibrariesTable();
    } else if (page === 'desktop' || page === 'downloads' || page === 'documents' || page === 'other-files') {
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
  view: 'grid', // 'grid' or 'list'
  size: 'medium' // 'small', 'medium', or 'large'
};

// Load view preferences from localStorage
function loadViewPreferences() {
  try {
    const stored = localStorage.getItem('viewPreferences');
    if (stored) {
      viewPreferences = JSON.parse(stored);
    }
  } catch (error) {
    console.error('Failed to load view preferences:', error);
  }
}

// Save view preferences to localStorage
function saveViewPreferences() {
  try {
    localStorage.setItem('viewPreferences', JSON.stringify(viewPreferences));
  } catch (error) {
    console.error('Failed to save view preferences:', error);
  }
}

// Apply view preferences to all file lists
function applyViewPreferences() {
  // Update all view buttons
  document.querySelectorAll('.view-btn').forEach(btn => {
    if (btn.dataset.view === viewPreferences.view) {
      btn.classList.add('active');
    } else {
      btn.classList.remove('active');
    }
  });
  
  // Update all size selects
  document.querySelectorAll('.view-size-select').forEach(select => {
    select.value = viewPreferences.size;
  });
  
  // Apply to all file lists
  applyViewToAllLists();
}

// Apply current view settings to all file lists
function applyViewToAllLists() {
  const allFileLists = [
    document.getElementById('results-list'),
    document.getElementById('desktop-file-list'),
    document.getElementById('downloads-file-list'),
    document.getElementById('documents-file-list'),
    document.getElementById('other-files-file-list')
  ].filter(el => el !== null);
  
  allFileLists.forEach(list => {
    // Apply view type
    if (viewPreferences.view === 'list') {
      list.classList.add('list-view');
      list.classList.remove('grid-view');
    } else {
      list.classList.add('grid-view');
      list.classList.remove('list-view');
    }
    
    // Apply size
    list.classList.remove('size-small', 'size-medium', 'size-large');
    list.classList.add(`size-${viewPreferences.size}`);
    
    // Update grid template columns for list view
    if (viewPreferences.view === 'list') {
      list.style.gridTemplateColumns = '1fr';
    } else {
      // Grid view - adjust based on size
      const minWidths = {
        small: '120px',
        medium: '180px',
        large: '240px'
      };
      list.style.gridTemplateColumns = `repeat(auto-fill, minmax(${minWidths[viewPreferences.size]}, 1fr))`;
    }
  });
  
  // Re-render current page if needed
  const activePage = document.querySelector('.page.active');
  if (activePage) {
    const pageId = activePage.id;
    if (pageId === 'search-page' && lastSearchResults.length > 0) {
      displayResults(lastSearchResults);
    } else if (['desktop-page', 'downloads-page', 'documents-page', 'other-files-page'].includes(pageId)) {
      // Reload current folder page
      const pageType = pageId.replace('-page', '');
      loadFolderPage(pageType);
    }
  }
}

// View Toggle (List/Grid) - Universal
document.addEventListener('click', (e) => {
  if (e.target.closest('.view-btn')) {
    const btn = e.target.closest('.view-btn');
    viewPreferences.view = btn.dataset.view;
    
    // Update all view buttons
    document.querySelectorAll('.view-btn').forEach(b => {
      if (b.dataset.view === viewPreferences.view) {
        b.classList.add('active');
      } else {
        b.classList.remove('active');
      }
    });
    
    saveViewPreferences();
    applyViewToAllLists();
  }
});

// Size Select - Universal
document.addEventListener('change', (e) => {
  if (e.target.classList.contains('view-size-select')) {
    viewPreferences.size = e.target.value;
    
    // Update all size selects
    document.querySelectorAll('.view-size-select').forEach(select => {
      select.value = viewPreferences.size;
    });
    
    saveViewPreferences();
    applyViewToAllLists();
  }
});

// Similarity Slider
const similaritySlider = document.getElementById('similarity-slider');
const similarityValue = document.getElementById('similarity-value');
let similarityThreshold = 30; // Lower default threshold for better results

if (similaritySlider) {
    similaritySlider.addEventListener('input', (e) => {
        similarityThreshold = parseInt(e.target.value);
        if (similarityValue) similarityValue.textContent = `${similarityThreshold}%`;
        
        // Filter results if we have them
        if (lastSearchResults.length > 0) {
            filterResultsBySimilarity();
        }
    });
}

function filterResultsBySimilarity() {
  const filtered = lastSearchResults.filter(r => r.similarity * 100 >= similarityThreshold);
  displayResults(filtered);
}

// Sort Select
const sortSelect = document.getElementById('sort-select');
if (sortSelect) {
    sortSelect.addEventListener('change', (e) => {
        if (lastSearchResults.length > 0) {
            sortResults(e.target.value);
        }
    });
}

function sortResults(sortBy) {
  const sorted = [...lastSearchResults];
  
  switch(sortBy) {
    case 'relevance':
      sorted.sort((a, b) => b.similarity - a.similarity);
      break;
    case 'date':
      // Currently backend doesn't return date, so we'll just keep order or mock it
      break;
    case 'type':
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
        const stored = localStorage.getItem('searchHistory');
        if (stored) {
            searchHistory = JSON.parse(stored);
        }
    } catch (error) {
        console.error('Failed to load search history:', error);
    }
}

// Save search history to localStorage
function saveSearchHistory() {
    try {
        localStorage.setItem('searchHistory', JSON.stringify(searchHistory));
    } catch (error) {
        console.error('Failed to save search history:', error);
    }
}

// Add to search history
function addToSearchHistory(query) {
    if (!query || query.trim().length === 0) return;
    
    // Remove if already exists
    searchHistory = searchHistory.filter(q => q.toLowerCase() !== query.toLowerCase());
    
    // Add to front
    searchHistory.unshift(query.trim());
    
    // Keep only last 10
    searchHistory = searchHistory.slice(0, 10);
    
    saveSearchHistory();
}

// Remove from search history
function removeFromSearchHistory(query) {
    searchHistory = searchHistory.filter(q => q.toLowerCase() !== query.toLowerCase());
    saveSearchHistory();
    // Refresh the dropdown if it's visible
    if (document.getElementById('search-history-dropdown')?.style.display === 'block') {
        showSearchHistory();
    }
}

// Show search history dropdown
function showSearchHistory() {
    if (!searchInput || searchHistory.length === 0) return;
    
    // Create dropdown if it doesn't exist
    let dropdown = document.getElementById('search-history-dropdown');
    if (!dropdown) {
        dropdown = document.createElement('div');
        dropdown.id = 'search-history-dropdown';
        dropdown.className = 'search-history-dropdown';
        // Append to the search input wrapper (parent element)
        const wrapper = searchInput.closest('.main-search-input-wrapper') || searchInput.parentElement;
        if (wrapper) {
            wrapper.style.position = 'relative';
            wrapper.appendChild(dropdown);
        }
    }
    
    dropdown.innerHTML = searchHistory.map((query, idx) => `
        <div class="history-item" data-query="${escapeHtml(query)}">
            <div class="history-item-content">
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="11" cy="11" r="8"></circle><line x1="21" y1="21" x2="16.65" y2="16.65"></line></svg>
                <span>${escapeHtml(query)}</span>
            </div>
            <button class="history-item-delete" data-query="${escapeHtml(query)}" title="Remove from history">
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="18" y1="6" x2="6" y2="18"></line><line x1="6" y1="6" x2="18" y2="18"></line></svg>
            </button>
        </div>
    `).join('');
    
    dropdown.style.display = 'block';
    
    // Add click handlers for selecting history item
    dropdown.querySelectorAll('.history-item-content').forEach(content => {
        content.addEventListener('click', (e) => {
            e.stopPropagation();
            const item = content.closest('.history-item');
            const query = item.dataset.query;
            searchInput.value = query;
            dropdown.style.display = 'none';
            performSearch();
        });
    });
    
    // Add click handlers for delete buttons
    dropdown.querySelectorAll('.history-item-delete').forEach(deleteBtn => {
        deleteBtn.addEventListener('click', (e) => {
            e.stopPropagation();
            const query = deleteBtn.dataset.query;
            removeFromSearchHistory(query);
        });
    });
}

// Hide search history dropdown
function hideSearchHistory() {
    const dropdown = document.getElementById('search-history-dropdown');
    if (dropdown) {
        setTimeout(() => {
            dropdown.style.display = 'none';
        }, 200);
    }
}

// Search functionality
const searchInput = document.getElementById('search-input');
const resultsList = document.getElementById('results-list');
const initialState = document.getElementById('initial-state');
const loadingState = document.getElementById('loading-state');
const noResults = document.getElementById('no-results');
const resultsCount = document.getElementById('results-count');
let lastSearchResults = [];

if (searchInput) {
    searchInput.addEventListener('keypress', (e) => {
        if (e.key === 'Enter') {
            performSearch();
            hideSearchHistory();
        }
    });
    
    searchInput.addEventListener('focus', () => {
        showSearchHistory();
    });
    
    searchInput.addEventListener('blur', () => {
        hideSearchHistory();
    });
    
    searchInput.addEventListener('input', () => {
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
  if (resultsList) resultsList.style.display = 'none';
  if (initialState) initialState.style.display = 'none';
  if (noResults) noResults.style.display = 'none';
  if (loadingState) loadingState.style.display = 'flex';
  if (resultsCount) resultsCount.textContent = 'Searching...';
  
  try {
    // First, parse the query to extract filters
    let parsedQuery = null;
    try {
      const parseResponse = await window.electronAPI.apiRequest('POST', '/api/search/parse', {
        query
      });
      if (parseResponse.success && parseResponse.data) {
        parsedQuery = parseResponse.data;
        activeFilters = parsedQuery.filters;
        // Display filters in UI
        displayFilters(parsedQuery.filters);
      }
    } catch (parseError) {
      console.warn('Failed to parse query:', parseError);
      // Continue with original query if parsing fails
      activeFilters = null;
      displayFilters(null);
    }
    
    // Perform search with parsed query and filters
    const searchQuery = parsedQuery ? parsedQuery.query : query;
    const response = await window.electronAPI.apiRequest('POST', '/api/search', { 
      query: searchQuery,
      limit: maxSearchResults,
      filters: activeFilters
    });
    
    if (response.success && response.data.results) {
      lastSearchResults = response.data.results;
      console.log('Search results received:', lastSearchResults.length, 'results');
      if (lastSearchResults.length > 0) {
        console.log('Sample similarities:', lastSearchResults.slice(0, 5).map(r => ({
          file: r.file_name,
          similarity: (r.similarity * 100).toFixed(1) + '%'
        })));
      }
      if (resultsCount) resultsCount.textContent = `Found ${lastSearchResults.length} relevant documents`;
      filterResultsBySimilarity();
    } else {
      showError('Search failed: ' + (response.error || 'Unknown error'));
      if (loadingState) loadingState.style.display = 'none';
      if (initialState) initialState.style.display = 'flex';
    }
  } catch (error) {
    showError('Search error: ' + error.message);
    if (loadingState) loadingState.style.display = 'none';
    if (initialState) initialState.style.display = 'flex';
  }
}

// Display active filters as chips
function displayFilters(filters) {
  const filterContainer = document.getElementById('filter-container');
  const filterChips = document.getElementById('filter-chips');
  
  if (!filterContainer || !filterChips) return;
  
  if (!filters || (!filters.date_range && !filters.file_types && !filters.folder_paths)) {
    filterContainer.style.display = 'none';
    filterChips.innerHTML = '';
    return;
  }
  
  filterContainer.style.display = 'flex';
  filterChips.innerHTML = '';
  
  // Date filter chip
  if (filters.date_range) {
    const dateRange = filters.date_range;
    let dateText = '';
    if (dateRange.month && dateRange.year) {
      const monthNames = ['January', 'February', 'March', 'April', 'May', 'June',
                         'July', 'August', 'September', 'October', 'November', 'December'];
      dateText = `${monthNames[dateRange.month - 1]} ${dateRange.year}`;
    } else if (dateRange.year) {
      dateText = `${dateRange.year}`;
    } else if (dateRange.start && dateRange.end) {
      const startDate = new Date(dateRange.start * 1000);
      const endDate = new Date(dateRange.end * 1000);
      dateText = `${startDate.toLocaleDateString()} - ${endDate.toLocaleDateString()}`;
    }
    
    if (dateText) {
      const chip = createFilterChip('date', dateText, () => {
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
    filters.file_types.forEach(fileType => {
      const chip = createFilterChip('file-type', fileType.toUpperCase(), () => {
        if (activeFilters && activeFilters.file_types) {
          activeFilters.file_types = activeFilters.file_types.filter(ft => ft !== fileType);
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
    filters.folder_paths.forEach(folder => {
      const chip = createFilterChip('folder', folder, () => {
        if (activeFilters && activeFilters.folder_paths) {
          activeFilters.folder_paths = activeFilters.folder_paths.filter(fp => fp !== folder);
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
  const chip = document.createElement('div');
  chip.className = `filter-chip filter-chip-${type}`;
  chip.innerHTML = `
    <span class="filter-chip-label">${escapeHtml(label)}</span>
    <button class="filter-chip-remove" title="Remove filter">×</button>
  `;
  
  chip.querySelector('.filter-chip-remove').addEventListener('click', (e) => {
    e.stopPropagation();
    onRemove();
  });
  
  return chip;
}

// Clear all filters
const clearFiltersBtn = document.getElementById('clear-filters-btn');
if (clearFiltersBtn) {
  clearFiltersBtn.addEventListener('click', () => {
    activeFilters = null;
    displayFilters(null);
    performSearch();
  });
}

async function displayResults(results) {
  if (!resultsList) return;

  // Hide loading
  if (loadingState) loadingState.style.display = 'none';

  if (results.length === 0) {
    if (noResults) noResults.style.display = 'flex';
    resultsList.style.display = 'none';
    return;
  }
  
  if (noResults) noResults.style.display = 'none';
  if (initialState) initialState.style.display = 'none';
  resultsList.style.display = 'grid';
  resultsList.innerHTML = '';
  
  // Apply view preferences
  applyViewToElement(resultsList);
  
  for (const result of results) {
    const item = document.createElement('div');
    item.className = 'result-item';
    
    const filePath = result.file_path;
    const fileName = result.file_name || filePath.split(/[\\/]/).pop();
    const fileIconData = getFileIcon(fileName);
    
    // Get file preview/description
    let description = '';
    try {
      const previewResult = await window.electronAPI.readFilePreview(filePath);
      if (previewResult.success && previewResult.isText) {
        const lines = previewResult.preview.split('\n').filter(l => l.trim());
        description = lines[0] ? lines[0].substring(0, 120) : previewResult.preview.substring(0, 120);
        if (description.length >= 120) description += '...';
      }
    } catch (error) {
      // Preview failed, continue
    }
    
    item.innerHTML = `
      <div class="result-header">
        <div class="file-icon-wrapper" data-file-type="${fileIconData.category}">${fileIconData.icon}</div>
        <div class="file-info">
          <div class="file-name" title="${escapeHtml(fileName)}">${escapeHtml(fileName)}</div>
          ${description ? `<div class="file-preview">${escapeHtml(description)}</div>` : ''}
        </div>
      </div>
      <div class="result-footer">
        <div class="file-path-tag" title="${escapeHtml(filePath)}">${escapeHtml(filePath)}</div>
        <div class="relevance-tag">${(result.similarity * 100).toFixed(0)}% Match</div>
      </div>
    `;
    
    item.addEventListener('click', async () => {
      await openFile(filePath);
    });
    
    resultsList.appendChild(item);
  }
}

function getFileExtension(filename) {
  if (!filename) return 'file';
  const parts = filename.split('.');
  return parts.length > 1 ? parts[parts.length - 1] : 'file';
}

// Get appropriate icon SVG and file type category for file type
function getFileIcon(filename, isDirectory = false) {
  if (isDirectory) {
    return {
      icon: `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"></path></svg>`,
      category: 'folder'
    };
  }
  
  const ext = getFileExtension(filename).toLowerCase();
  
  // PDF - Red
  if (ext === 'pdf') {
    return {
      icon: `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"></path><polyline points="14 2 14 8 20 8"></polyline><line x1="16" y1="13" x2="8" y2="13"></line><line x1="16" y1="17" x2="8" y2="17"></line><polyline points="10 9 9 9 8 9"></polyline></svg>`,
      category: 'pdf'
    };
  }
  
  // DOC, DOCX, XLS - Blue
  if (['doc', 'docx', 'xls'].includes(ext)) {
    return {
      icon: `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"></path><polyline points="14 2 14 8 20 8"></polyline><line x1="16" y1="13" x2="8" y2="13"></line><line x1="16" y1="17" x2="8" y2="17"></line><line x1="10" y1="9" x2="8" y2="9"></line></svg>`,
      category: 'office'
    };
  }
  
  // TXT - Grey
  if (ext === 'txt') {
    return {
      icon: `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"></path><polyline points="14 2 14 8 20 8"></polyline><line x1="16" y1="13" x2="8" y2="13"></line><line x1="16" y1="17" x2="8" y2="17"></line></svg>`,
      category: 'text'
    };
  }
  
  // Other document types
  const documentTypes = {
    'xlsx': `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"></path><polyline points="14 2 14 8 20 8"></polyline><line x1="16" y1="13" x2="8" y2="13"></line><line x1="16" y1="17" x2="8" y2="17"></line><line x1="10" y1="9" x2="8" y2="9"></line></svg>`,
    'rtf': `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"></path><polyline points="14 2 14 8 20 8"></polyline><line x1="16" y1="13" x2="8" y2="13"></line><line x1="16" y1="17" x2="8" y2="17"></line></svg>`,
  };
  
  // Code/Text files
  const codeTypes = {
    'js': `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="16 18 22 12 16 6"></polyline><polyline points="8 6 2 12 8 18"></polyline></svg>`,
    'ts': `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="16 18 22 12 16 6"></polyline><polyline points="8 6 2 12 8 18"></polyline></svg>`,
    'jsx': `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="16 18 22 12 16 6"></polyline><polyline points="8 6 2 12 8 18"></polyline></svg>`,
    'tsx': `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="16 18 22 12 16 6"></polyline><polyline points="8 6 2 12 8 18"></polyline></svg>`,
    'py': `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="16 18 22 12 16 6"></polyline><polyline points="8 6 2 12 8 18"></polyline></svg>`,
    'java': `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="16 18 22 12 16 6"></polyline><polyline points="8 6 2 12 8 18"></polyline></svg>`,
    'cpp': `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="16 18 22 12 16 6"></polyline><polyline points="8 6 2 12 8 18"></polyline></svg>`,
    'c': `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="16 18 22 12 16 6"></polyline><polyline points="8 6 2 12 8 18"></polyline></svg>`,
    'rs': `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="16 18 22 12 16 6"></polyline><polyline points="8 6 2 12 8 18"></polyline></svg>`,
    'go': `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="16 18 22 12 16 6"></polyline><polyline points="8 6 2 12 8 18"></polyline></svg>`,
    'html': `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="16 18 22 12 16 6"></polyline><polyline points="8 6 2 12 8 18"></polyline></svg>`,
    'css': `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="16 18 22 12 16 6"></polyline><polyline points="8 6 2 12 8 18"></polyline></svg>`,
    'json': `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="16 18 22 12 16 6"></polyline><polyline points="8 6 2 12 8 18"></polyline></svg>`,
    'xml': `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="16 18 22 12 16 6"></polyline><polyline points="8 6 2 12 8 18"></polyline></svg>`,
    'yaml': `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="16 18 22 12 16 6"></polyline><polyline points="8 6 2 12 8 18"></polyline></svg>`,
    'yml': `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="16 18 22 12 16 6"></polyline><polyline points="8 6 2 12 8 18"></polyline></svg>`,
    'md': `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"></path><polyline points="14 2 14 8 20 8"></polyline><line x1="16" y1="13" x2="8" y2="13"></line><line x1="16" y1="17" x2="8" y2="17"></line><line x1="10" y1="9" x2="8" y2="9"></line></svg>`,
  };
  
  // Image types
  const imageTypes = {
    'jpg': `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="18" height="18" rx="2" ry="2"></rect><circle cx="8.5" cy="8.5" r="1.5"></circle><polyline points="21 15 16 10 5 21"></polyline></svg>`,
    'jpeg': `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="18" height="18" rx="2" ry="2"></rect><circle cx="8.5" cy="8.5" r="1.5"></circle><polyline points="21 15 16 10 5 21"></polyline></svg>`,
    'png': `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="18" height="18" rx="2" ry="2"></rect><circle cx="8.5" cy="8.5" r="1.5"></circle><polyline points="21 15 16 10 5 21"></polyline></svg>`,
    'gif': `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="18" height="18" rx="2" ry="2"></rect><circle cx="8.5" cy="8.5" r="1.5"></circle><polyline points="21 15 16 10 5 21"></polyline></svg>`,
    'svg': `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="18" height="18" rx="2" ry="2"></rect><circle cx="8.5" cy="8.5" r="1.5"></circle><polyline points="21 15 16 10 5 21"></polyline></svg>`,
    'webp': `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="18" height="18" rx="2" ry="2"></rect><circle cx="8.5" cy="8.5" r="1.5"></circle><polyline points="21 15 16 10 5 21"></polyline></svg>`,
    'bmp': `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="18" height="18" rx="2" ry="2"></rect><circle cx="8.5" cy="8.5" r="1.5"></circle><polyline points="21 15 16 10 5 21"></polyline></svg>`,
    'ico': `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="18" height="18" rx="2" ry="2"></rect><circle cx="8.5" cy="8.5" r="1.5"></circle><polyline points="21 15 16 10 5 21"></polyline></svg>`,
  };
  
  // Media types
  const mediaTypes = {
    'mp4': `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="5 3 19 12 5 21 5 3"></polygon></svg>`,
    'avi': `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="5 3 19 12 5 21 5 3"></polygon></svg>`,
    'mov': `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="5 3 19 12 5 21 5 3"></polygon></svg>`,
    'mp3': `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5"></polygon><path d="M19.07 4.93a10 10 0 0 1 0 14.14M15.54 8.46a5 5 0 0 1 0 7.07"></path></svg>`,
    'wav': `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5"></polygon><path d="M19.07 4.93a10 10 0 0 1 0 14.14M15.54 8.46a5 5 0 0 1 0 7.07"></path></svg>`,
    'flac': `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5"></polygon><path d="M19.07 4.93a10 10 0 0 1 0 14.14M15.54 8.46a5 5 0 0 1 0 7.07"></path></svg>`,
    'ogg': `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5"></polygon><path d="M19.07 4.93a10 10 0 0 1 0 14.14M15.54 8.46a5 5 0 0 1 0 7.07"></path></svg>`,
  };
  
  // Archive types
  const archiveTypes = {
    'zip': `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z"></path><polyline points="3.27 6.96 12 12.01 20.73 6.96"></polyline><line x1="12" y1="22.08" x2="12" y2="12"></line></svg>`,
    'rar': `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z"></path><polyline points="3.27 6.96 12 12.01 20.73 6.96"></polyline><line x1="12" y1="22.08" x2="12" y2="12"></line></svg>`,
    '7z': `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z"></path><polyline points="3.27 6.96 12 12.01 20.73 6.96"></polyline><line x1="12" y1="22.08" x2="12" y2="12"></line></svg>`,
    'tar': `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z"></path><polyline points="3.27 6.96 12 12.01 20.73 6.96"></polyline><line x1="12" y1="22.08" x2="12" y2="12"></line></svg>`,
    'gz': `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z"></path><polyline points="3.27 6.96 12 12.01 20.73 6.96"></polyline><line x1="12" y1="22.08" x2="12" y2="12"></line></svg>`,
  };
  
  // Check each category
  if (documentTypes[ext]) return { icon: documentTypes[ext], category: 'document' };
  if (codeTypes[ext]) return { icon: codeTypes[ext], category: 'code' };
  if (imageTypes[ext]) return { icon: imageTypes[ext], category: 'image' };
  if (mediaTypes[ext]) return { icon: mediaTypes[ext], category: 'media' };
  if (archiveTypes[ext]) return { icon: archiveTypes[ext], category: 'archive' };
  
  // Default file icon
  return {
    icon: `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"></path><polyline points="14 2 14 8 20 8"></polyline></svg>`,
    category: 'file'
  };
}

async function openFile(filePath) {
  try {
    const result = await window.electronAPI.openFile(filePath);
    if (!result.success) {
      alert('Failed to open file: ' + (result.error || 'Unknown error'));
    }
  } catch (error) {
    showError('Error opening file: ' + error.message);
  }
}

function escapeHtml(text) {
  if (!text) return '';
  const div = document.createElement('div');
  div.textContent = text;
  return div.innerHTML;
}

function showError(message) {
  console.error(message);
}

// Settings functionality
let maxSearchResults = 100; // Default value

async function loadSettings() {
  try {
    const response = await window.electronAPI.apiRequest('GET', '/api/settings');
    if (response.success) {
      const settings = response.data;
      
      // Load performance mode
      const modeRadio = document.querySelector(`input[name="performance-mode"][value="${settings.performance_mode}"]`);
      if (modeRadio) {
        modeRadio.checked = true;
        // Update active class on parent card
        document.querySelectorAll('.model-card').forEach(card => card.classList.remove('active'));
        const parentCard = modeRadio.closest('.model-card');
        if (parentCard) parentCard.classList.add('active');
      }
      
      // Load max search results
      if (settings.max_search_results) {
        maxSearchResults = settings.max_search_results;
        const slider = document.getElementById('max-results-slider');
        const valueDisplay = document.getElementById('max-results-value');
        if (slider) {
          slider.value = maxSearchResults;
        }
        if (valueDisplay) {
          valueDisplay.textContent = maxSearchResults;
        }
      }
    }
  } catch (error) {
    console.error('Failed to load settings:', error);
  }
}

// Model card selection logic
document.querySelectorAll('.model-card').forEach(card => {
    card.addEventListener('click', () => {
        const radio = card.querySelector('input[type="radio"]');
        if (radio) {
            radio.checked = true;
            document.querySelectorAll('.model-card').forEach(c => c.classList.remove('active'));
            card.classList.add('active');
        }
    });
});

async function loadSystemInfo() {
  const systemInfoDiv = document.getElementById('system-info');
  if (!systemInfoDiv) return;

  try {
    const response = await window.electronAPI.apiRequest('GET', '/api/system-info');
    if (response.success) {
      const info = response.data;
      systemInfoDiv.innerHTML = `
        <div style="display:grid; grid-template-columns: 1fr 1fr; gap: 1rem;">
            <div>
                <div style="font-size:0.75rem; text-transform:uppercase; color:var(--text-muted);">RAM</div>
                <div style="font-weight:600;">${info.total_ram_mb}MB Total</div>
                <div style="font-size:0.875rem;">${info.available_ram_mb}MB Available</div>
            </div>
            <div>
                <div style="font-size:0.75rem; text-transform:uppercase; color:var(--text-muted);">Processor</div>
                <div style="font-weight:600;">${info.cpu_cores} Active Cores</div>
                <div style="font-size:0.875rem;">Mode: ${info.current_mode || 'Standard'}</div>
            </div>
        </div>
      `;
    }
  } catch (error) {
    systemInfoDiv.innerHTML = '<p>Failed to load system information.</p>';
  }
}

async function loadLibrariesTable() {
  const tableBody = document.getElementById('libraries-table-body');
  if (!tableBody) return;

  try {
    const response = await window.electronAPI.apiRequest('GET', '/api/settings');
    if (response.success && response.data.indexed_directories) {
      const dirs = response.data.indexed_directories;
      
      if (dirs.length === 0) {
        tableBody.innerHTML = '<tr><td colspan="5" style="text-align: center; padding: 3rem;">No folders indexed yet. Add a folder to enable semantic search.</td></tr>';
      } else {
        const filesResponse = await window.electronAPI.apiRequest('GET', '/api/files');
        const fileCounts = {};
        
        if (filesResponse.success && filesResponse.data.files) {
          filesResponse.data.files.forEach(file => {
            const dir = dirs.find(d => file.file_path.startsWith(d));
            if (dir) {
              fileCounts[dir] = (fileCounts[dir] || 0) + 1;
            }
          });
        }
        
        tableBody.innerHTML = dirs.map(dir => `
          <tr>
            <td style="max-width:300px; overflow:hidden; text-overflow:ellipsis;" title="${escapeHtml(dir)}">${escapeHtml(dir)}</td>
            <td>${fileCounts[dir] || 0} files</td>
            <td>Recently</td>
            <td><span class="status-badge synced">Synced</span></td>
            <td style="text-align:right;">
              <button class="btn-secondary" style="padding: 0.25rem 0.75rem; font-size: 0.75rem;" onclick="removeDirectory('${escapeHtml(dir)}')">Remove</button>
            </td>
          </tr>
        `).join('');
      }
    }
  } catch (error) {
    tableBody.innerHTML = '<tr><td colspan="5" style="text-align: center; padding: 2rem;">Error loading libraries.</td></tr>';
  }
}

async function removeDirectory(path) {
  try {
    const response = await window.electronAPI.apiRequest('GET', '/api/settings');
    if (response.success) {
      const dirs = response.data.indexed_directories.filter(d => d !== path);
      await window.electronAPI.apiRequest('PUT', '/api/settings', {
        indexed_directories: dirs
      });
      loadLibrariesTable();
    }
  } catch (error) {
    console.error('Failed to remove directory:', error);
  }
}

window.removeDirectory = removeDirectory;

// Max results slider handler
const maxResultsSlider = document.getElementById('max-results-slider');
const maxResultsValue = document.getElementById('max-results-value');
if (maxResultsSlider && maxResultsValue) {
    maxResultsSlider.addEventListener('input', (e) => {
        const value = parseInt(e.target.value);
        maxResultsValue.textContent = value;
        maxSearchResults = value;
    });
}

const saveSettingsBtn = document.getElementById('save-settings');
if (saveSettingsBtn) {
    saveSettingsBtn.addEventListener('click', async () => {
        const checkedMode = document.querySelector('input[name="performance-mode"]:checked');
        if (!checkedMode) return;
        
        const selectedMode = checkedMode.value;
        const messageDiv = document.getElementById('settings-message');
        
        // Get max search results from slider
        const maxResults = maxResultsSlider ? parseInt(maxResultsSlider.value) : maxSearchResults;
        
        try {
            const response = await window.electronAPI.apiRequest('PUT', '/api/settings', {
                performance_mode: selectedMode,
                max_search_results: maxResults
            });
            
            if (response.success) {
                maxSearchResults = maxResults; // Update local variable
                messageDiv.textContent = 'Settings applied! Backend will adapt on next index.';
                messageDiv.style.color = 'var(--accent-primary)';
            } else {
                messageDiv.textContent = 'Error: ' + (response.error || 'Unknown error');
                messageDiv.style.color = '#dc2626';
            }
        } catch (error) {
            messageDiv.textContent = 'Connection error: ' + error.message;
            messageDiv.style.color = '#dc2626';
        }
    });
}

const clearIndexBtn = document.getElementById('clear-index');
if (clearIndexBtn) {
    clearIndexBtn.addEventListener('click', async () => {
        const confirmClear = confirm('Purge Index? This deletes all embeddings but keeps folders in the list. Your files remain untouched. Search will not work until re-indexing.');
        if (!confirmClear) return;
        
        try {
            // Clear the index (files and embeddings) but keep folders in settings
            const response = await window.electronAPI.apiRequest('POST', '/api/index/clear');
            if (response.success) {
                showSuccess('Index purged. You can re-index folders by clicking "Add Folder" again.');
                loadLibrariesTable();
            }
        } catch (error) {
            showError('Purge failed: ' + error.message);
        }
    });
}

const addDirectoryBtn = document.getElementById('add-directory');
if (addDirectoryBtn) {
    addDirectoryBtn.addEventListener('click', async () => {
        try {
            const result = await window.electronAPI.selectDirectory();
            if (result.success && result.path) {
                const settingsResponse = await window.electronAPI.apiRequest('GET', '/api/settings');
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
                        const response = await window.electronAPI.apiRequest('PUT', '/api/settings', {
                            indexed_directories: updatedDirs
                        });
                        if (!response.success) {
                            showError('Failed to update settings: ' + (response.error || 'Unknown error'));
                            return;
                        }
                    }
                    
                    // Always start indexing (even if already in list, allows re-indexing)
                    await window.electronAPI.apiRequest('POST', '/api/index/start', {
                        directory: result.path
                    });
                    loadLibrariesTable();
                    // Start polling for indexing progress
                    startIndexingProgressPoll();
                }
            }
        } catch (error) {
            console.error('Failed to add directory:', error);
            showError('Failed to add directory: ' + error.message);
        }
    });
}

// Backend Health
async function checkBackendConnection() {
  try {
    const response = await window.electronAPI.apiRequest('GET', '/api/health');
    if (response.success) {
      updateSidebarStatus('System Ready', 100);
    } else {
      updateSidebarStatus('Backend Offline', 0);
    }
  } catch (error) {
    updateSidebarStatus('Connecting...', 0);
  }
}

function updateSidebarStatus(text, progress) {
  const statusText = document.getElementById('sidebar-progress-text');
  const statusFill = document.getElementById('sidebar-progress');
  if (statusText) statusText.textContent = text;
  if (statusFill) statusFill.style.width = `${progress}%`;
}

let indexingProgressInterval = null;

async function checkIndexingProgress() {
  try {
    const response = await window.electronAPI.apiRequest('GET', '/api/index/status');
    if (response.success && response.data) {
      const status = response.data;
      if (status.is_indexing && status.current !== null && status.total !== null) {
        const percent = status.total > 0 ? Math.round((status.current / status.total) * 100) : 0;
        const fileName = status.current_file ? status.current_file.split(/[\\/]/).pop() : '';
        updateSidebarStatus(
          `Indexing: ${status.current}/${status.total} ${fileName ? `(${fileName})` : ''}`,
          percent
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
let currentBrowserPath = '';
let selectedBrowserItem = null;
let specialFolders = {};

// Load special folders on startup
async function loadSpecialFolders() {
    try {
        const response = await window.electronAPI.apiRequest('GET', '/api/files/special-folders');
        if (response.success && response.data) {
            specialFolders = response.data;
        }
    } catch (error) {
        console.error('Failed to load special folders:', error);
    }
}

// Load folder page based on page type
async function loadFolderPage(pageType) {
    let folderPath = null;
    let fileListId = null;
    
    switch(pageType) {
        case 'desktop':
            folderPath = specialFolders.desktop;
            fileListId = 'desktop-file-list';
            break;
        case 'downloads':
            folderPath = specialFolders.downloads;
            fileListId = 'downloads-file-list';
            break;
        case 'documents':
            folderPath = specialFolders.documents;
            fileListId = 'documents-file-list';
            break;
        case 'other-files':
            folderPath = specialFolders.home;
            fileListId = 'other-files-file-list';
            break;
    }
    
    if (!folderPath) {
        // Load special folders if not already loaded
        await loadSpecialFolders();
        switch(pageType) {
            case 'desktop':
                folderPath = specialFolders.desktop;
                break;
            case 'downloads':
                folderPath = specialFolders.downloads;
                break;
            case 'documents':
                folderPath = specialFolders.documents;
                break;
            case 'other-files':
                folderPath = specialFolders.home;
                break;
        }
    }
    
    if (folderPath && fileListId) {
        await loadFolderFiles(folderPath, fileListId);
    }
}

// Load files for a specific folder and display them
async function loadFolderFiles(folderPath, fileListId) {
    try {
        const response = await window.electronAPI.apiRequest('GET', `/api/files/browse?path=${encodeURIComponent(folderPath)}`);
        if (response.success && response.data) {
            displayFolderFiles(response.data.items, fileListId);
        } else {
            showToast('Failed to load folder: ' + (response.error || 'Unknown error'), 'error');
        }
    } catch (error) {
        console.error('Failed to load folder:', error);
        showToast('Failed to load folder: ' + error.message, 'error');
    }
}

// Display files in a folder list
function displayFolderFiles(items, fileListId) {
    const fileList = document.getElementById(fileListId);
    if (!fileList) return;

    if (items.length === 0) {
        fileList.innerHTML = '<div class="empty-state"><h3>Folder is empty</h3></div>';
        return;
    }

    // Sort directories first, then files, both alphabetically
    items.sort((a, b) => {
        if (a.is_directory && !b.is_directory) return -1;
        if (!a.is_directory && b.is_directory) return 1;
        return a.name.localeCompare(b.name);
    });

    // Determine if list view
    const isListView = viewPreferences.view === 'list';

    fileList.innerHTML = items.map(item => {
        const iconData = getFileIcon(item.name, item.is_directory);
        const size = item.size ? formatFileSize(item.size) : '';
        const date = item.modified_time ? new Date(item.modified_time * 1000).toLocaleDateString() : '';

        if (isListView) {
            // Compact list view layout
            return `
                <div class="file-item" data-path="${escapeHtml(item.path)}" data-is-dir="${item.is_directory}">
                    <div class="file-icon-wrapper" data-file-type="${iconData.category}">${iconData.icon}</div>
                    <div class="file-item-name" title="${escapeHtml(item.name)}">${escapeHtml(item.name)}</div>
                    ${size ? `<div class="file-item-details">${size}</div>` : ''}
                    ${date ? `<div class="file-item-details">${date}</div>` : ''}
                </div>
            `;
        } else {
            // Grid view layout
            return `
                <div class="file-item" data-path="${escapeHtml(item.path)}" data-is-dir="${item.is_directory}">
                    <div class="file-icon-wrapper" data-file-type="${iconData.category}">${iconData.icon}</div>
                    <div class="file-item-name" title="${escapeHtml(item.name)}">${escapeHtml(item.name)}</div>
                    ${size ? `<div class="file-item-details">${size}</div>` : ''}
                    ${date ? `<div class="file-item-details">${date}</div>` : ''}
                </div>
            `;
        }
    }).join('');

    // Apply view preferences to the list
    applyViewToElement(fileList);

    // Add click handlers
    fileList.querySelectorAll('.file-item').forEach(item => {
        item.addEventListener('click', async () => {
            const filePath = item.dataset.path;
            const isDir = item.dataset.isDir === 'true';
            
            if (isDir) {
                // For directories, navigate into them (could be enhanced later)
                showToast('Double-click to open folder', 'info');
            } else {
                // Open file
                await openFile(filePath);
            }
        });
        
        item.addEventListener('dblclick', async () => {
            const filePath = item.dataset.path;
            const isDir = item.dataset.isDir === 'true';
            
            if (isDir) {
                // Navigate into directory - for now just show a message
                showToast('Folder navigation coming soon', 'info');
            } else {
                await openFile(filePath);
            }
        });
    });
}

// Apply view preferences to a specific element
function applyViewToElement(element) {
    if (!element) return;
    
    // Apply view type
    if (viewPreferences.view === 'list') {
        element.classList.add('list-view');
        element.classList.remove('grid-view');
        element.style.gridTemplateColumns = '1fr';
    } else {
        element.classList.add('grid-view');
        element.classList.remove('list-view');
        const minWidths = {
            small: '120px',
            medium: '180px',
            large: '240px'
        };
        element.style.gridTemplateColumns = `repeat(auto-fill, minmax(${minWidths[viewPreferences.size]}, 1fr))`;
    }
    
    // Apply size
    element.classList.remove('size-small', 'size-medium', 'size-large');
    element.classList.add(`size-${viewPreferences.size}`);
}

// Browse directory
async function browseDirectory(path) {
    try {
        const response = await window.electronAPI.apiRequest('GET', `/api/files/browse?path=${encodeURIComponent(path)}`);
        if (response.success && response.data) {
            currentBrowserPath = response.data.path;
            const pathEl = document.getElementById('browser-path');
            if (pathEl) pathEl.textContent = response.data.path;
            displayBrowserItems(response.data.items);
        }
    } catch (error) {
        console.error('Failed to browse directory:', error);
        showError('Failed to browse directory: ' + error.message);
    }
}

function displayBrowserItems(items) {
    const fileList = document.getElementById('browser-file-list');
    if (!fileList) return;

    if (items.length === 0) {
        fileList.innerHTML = '<div class="empty-state"><h3>Folder is empty</h3></div>';
        return;
    }

    fileList.innerHTML = items.map(item => {
        const iconData = getFileIcon(item.name, item.is_directory);
        
        const size = item.size ? formatFileSize(item.size) : '';
        const date = item.modified_time ? new Date(item.modified_time * 1000).toLocaleDateString() : '';

        return `
            <div class="browser-file-item" data-path="${escapeHtml(item.path)}" data-is-dir="${item.is_directory}">
                <div class="browser-file-icon" data-file-type="${iconData.category}">${iconData.icon}</div>
                <div class="browser-file-name" title="${escapeHtml(item.name)}">${escapeHtml(item.name)}</div>
                ${size || date ? `<div class="browser-file-info">${size} ${date}</div>` : ''}
            </div>
        `;
    }).join('');

    // Add click handlers
    fileList.querySelectorAll('.browser-file-item').forEach(item => {
        item.addEventListener('click', (e) => {
            // Remove previous selection
            fileList.querySelectorAll('.browser-file-item').forEach(i => i.classList.remove('selected'));
            item.classList.add('selected');
            selectedBrowserItem = {
                path: item.dataset.path,
                isDirectory: item.dataset.isDir === 'true'
            };
            
            // Enable/disable toolbar buttons
            const deleteBtn = document.getElementById('browser-delete');
            const renameBtn = document.getElementById('browser-rename');
            const addIndexBtn = document.getElementById('browser-add-to-index');
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
    if (bytes < 1024) return bytes + ' B';
    if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + ' KB';
    return (bytes / (1024 * 1024)).toFixed(1) + ' MB';
}

// Quick access handlers
document.querySelectorAll('.quick-access-item').forEach(btn => {
    btn.addEventListener('click', async () => {
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
const browserBackBtn = document.getElementById('browser-back');
if (browserBackBtn) {
    browserBackBtn.addEventListener('click', () => {
        if (currentBrowserPath) {
            const parts = currentBrowserPath.split(/[\\/]/).filter(p => p);
            if (parts.length > 1) {
                parts.pop();
                const parent = parts.join('/') || (currentBrowserPath.includes('\\') ? parts.join('\\') : '/');
                browseDirectory(parent);
            }
        }
    });
}

const browserNewFolderBtn = document.getElementById('browser-new-folder');
if (browserNewFolderBtn) {
    browserNewFolderBtn.addEventListener('click', async () => {
        const name = prompt('Enter folder name:');
        if (!name) return;
        
        try {
            const response = await window.electronAPI.apiRequest('POST', '/api/files/create-folder', {
                path: currentBrowserPath || specialFolders.home || '',
                name: name
            });
            if (response.success) {
                await browseDirectory(currentBrowserPath);
            } else {
                showError('Failed to create folder: ' + (response.error || 'Unknown error'));
            }
        } catch (error) {
            showError('Failed to create folder: ' + error.message);
        }
    });
}

const browserDeleteBtn = document.getElementById('browser-delete');
if (browserDeleteBtn) {
    browserDeleteBtn.addEventListener('click', async () => {
        if (!selectedBrowserItem) return;
        
        const itemName = selectedBrowserItem.path.split(/[\\/]/).pop();
        const confirmMsg = selectedBrowserItem.isDirectory 
            ? `Delete folder "${itemName}" and all its contents?`
            : `Delete file "${itemName}"?`;
        
        if (!confirm(confirmMsg)) return;
        
        try {
            const response = await window.electronAPI.apiRequest('POST', '/api/files/delete', {
                path: selectedBrowserItem.path
            });
            if (response.success) {
                selectedBrowserItem = null;
                await browseDirectory(currentBrowserPath);
            } else {
                showError('Failed to delete: ' + (response.error || 'Unknown error'));
            }
        } catch (error) {
            showError('Failed to delete: ' + error.message);
        }
    });
}

const browserRenameBtn = document.getElementById('browser-rename');
if (browserRenameBtn) {
    browserRenameBtn.addEventListener('click', async () => {
        if (!selectedBrowserItem) return;
        
        const oldName = selectedBrowserItem.path.split(/[\\/]/).pop();
        const newName = prompt('Enter new name:', oldName);
        if (!newName || newName === oldName) return;
        
        try {
            const response = await window.electronAPI.apiRequest('PUT', '/api/files/rename', {
                path: selectedBrowserItem.path,
                new_name: newName
            });
            if (response.success) {
                selectedBrowserItem = null;
                await browseDirectory(currentBrowserPath);
            } else {
                showError('Failed to rename: ' + (response.error || 'Unknown error'));
            }
        } catch (error) {
            showError('Failed to rename: ' + error.message);
        }
    });
}

const browserAddIndexBtn = document.getElementById('browser-add-to-index');
if (browserAddIndexBtn) {
    browserAddIndexBtn.addEventListener('click', async () => {
        if (!selectedBrowserItem || !selectedBrowserItem.isDirectory) return;
        
        try {
            // Add to indexed directories
            const settingsResponse = await window.electronAPI.apiRequest('GET', '/api/settings');
            if (settingsResponse.success) {
                const currentDirs = settingsResponse.data.indexed_directories || [];
                if (!currentDirs.includes(selectedBrowserItem.path)) {
                    const response = await window.electronAPI.apiRequest('PUT', '/api/settings', {
                        indexed_directories: [...currentDirs, selectedBrowserItem.path]
                    });
                    
                    if (response.success) {
                        await window.electronAPI.apiRequest('POST', '/api/index/start', {
                            directory: selectedBrowserItem.path
                        });
                        showSuccess('Folder added to index and indexing started!');
                        startIndexingProgressPoll();
                    }
                } else {
                    showInfo('Folder is already indexed');
                }
            }
        } catch (error) {
            showError('Failed to add to index: ' + error.message);
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
