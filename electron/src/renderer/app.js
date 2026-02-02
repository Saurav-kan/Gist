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
    alert('Error opening file: ' + error.message);
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
        const confirmClear = confirm('Purge Index? This deletes all embeddings but keeps your files. Search will not work until re-indexing.');
        if (!confirmClear) return;
        
        try {
            const response = await window.electronAPI.apiRequest('POST', '/api/index/clear');
            if (response.success) {
                alert('Index purged.');
                loadLibrariesTable();
            }
        } catch (error) {
            alert('Purge failed: ' + error.message);
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
                    if (!currentDirs.includes(result.path)) {
                        const response = await window.electronAPI.apiRequest('PUT', '/api/settings', {
                            indexed_directories: [...currentDirs, result.path]
                        });
                        
                        if (response.success) {
                            await window.electronAPI.apiRequest('POST', '/api/index/start', {
                                directory: result.path
                            });
                            loadLibrariesTable();
                        }
                    }
                }
            }
        } catch (error) {
            console.error('Failed to add directory:', error);
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

// Initialize
checkBackendConnection();
setInterval(checkBackendConnection, 10000);
loadSettings();
loadSystemInfo();
loadLibrariesTable();
