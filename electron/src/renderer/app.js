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
        currentFolder.textContent = page.charAt(0).toUpperCase() + page.slice(1);
    }
    
    // Load data when switching pages
    if (page === 'indexing') {
      loadSettings();
      loadSystemInfo();
      loadLibrariesTable();
    }
  });
});

// View Toggle (List/Grid)
let currentView = 'grid';
document.querySelectorAll('.view-btn').forEach(btn => {
  btn.addEventListener('click', () => {
    currentView = btn.dataset.view;
    document.querySelectorAll('.view-btn').forEach(b => b.classList.remove('active'));
    btn.classList.add('active');
    
    const resultsList = document.getElementById('results-list');
    if (!resultsList) return;

    if (currentView === 'list') {
      resultsList.classList.add('list-view');
      // For list view, we might want to change the grid layout to single column
      resultsList.style.gridTemplateColumns = '1fr';
    } else {
      resultsList.classList.remove('list-view');
      resultsList.style.gridTemplateColumns = '';
    }
    
    // Re-render results with new view if needed
    if (lastSearchResults.length > 0) {
      displayResults(lastSearchResults);
    }
  });
});

// Similarity Slider
const similaritySlider = document.getElementById('similarity-slider');
const similarityValue = document.getElementById('similarity-value');
let similarityThreshold = 70;

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
        }
    });
}

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
    const response = await window.electronAPI.apiRequest('POST', '/api/search', { 
      query,
      limit: maxSearchResults  // Use configured max results
    });
    
    if (response.success && response.data.results) {
      lastSearchResults = response.data.results;
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
  
  for (const result of results) {
    const item = document.createElement('div');
    item.className = 'result-item';
    
    const filePath = result.file_path;
    const fileName = result.file_name || filePath.split(/[\\/]/).pop();
    const fileExt = getFileExtension(fileName);
    const fileIcon = fileExt.toUpperCase().substring(0, 3);
    
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
        <div class="file-icon-wrapper">${fileIcon}</div>
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
        const icon = item.is_directory ? 
            '<svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"></path></svg>' :
            '<svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"></path><polyline points="14 2 14 8 20 8"></polyline></svg>';
        
        const size = item.size ? formatFileSize(item.size) : '';
        const date = item.modified_time ? new Date(item.modified_time * 1000).toLocaleDateString() : '';

        return `
            <div class="browser-file-item" data-path="${escapeHtml(item.path)}" data-is-dir="${item.is_directory}">
                <div class="browser-file-icon">${icon}</div>
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

// Load browser data when switching to browser page
document.querySelectorAll('.nav-item').forEach(item => {
    item.addEventListener('click', () => {
        if (item.dataset.page === 'browser') {
            if (!currentBrowserPath && specialFolders.home) {
                browseDirectory(specialFolders.home);
            } else if (!currentBrowserPath) {
                loadSpecialFolders().then(() => {
                    if (specialFolders.home) {
                        browseDirectory(specialFolders.home);
                    }
                });
            }
        }
    });
});

// Initialize
checkBackendConnection();
setInterval(checkBackendConnection, 10000);
// Also check indexing progress periodically
setInterval(checkIndexingProgress, 2000);
loadSettings();
loadSystemInfo();
loadLibrariesTable();
loadSpecialFolders();
