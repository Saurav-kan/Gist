
// Onboarding Logic

document.addEventListener('DOMContentLoaded', () => {
    checkSetupStatus();
});

async function checkSetupStatus() {
    try {
        const response = await window.electronAPI.apiRequest('GET', '/api/setup/status');
        
        if (response.success && response.data) {
            const status = response.data.data ? response.data.data : response.data; // Handle wrapped/unwrapped
            handleSetupStatus(status);
        } else {
            console.error("Failed to check setup status:", response);
        }
    } catch (error) {
        console.error("Error checking setup status:", error);
    }
}

function handleSetupStatus(status) {
    console.log("Setup Status:", status);
    
    // Check if we need to show onboarding
    // We show it if Ollama is running BUT the required embedding model is missing.
    // If Ollama is NOT running, we might show a different error (or the backend might handle it).
    // For now, let's assume if Ollama is running but models are missing, we help.
    
    if (status.ollama_running && !status.embedding_model_installed) {
        showOnboardingModal(status);
    }
}

function showOnboardingModal(status) {
    const modal = document.getElementById('onboarding-modal');
    if (!modal) return;
    
    // Update content based on recommendations
    const ramText = document.getElementById('onboarding-ram-text');
    const modelText = document.getElementById('onboarding-model-text');
    
    if (ramText) ramText.textContent = `${status.system_ram_gb} GB`;
    if (modelText) modelText.textContent = status.recommended_embedding_model;
    
    window.recommendedEmbeddingModel = status.recommended_embedding_model;
    
    modal.classList.add('visible');
}

// Exposed to be called from HTML button
window.startOnboardingDownload = async function() {
    const btn = document.getElementById('onboarding-start-btn');
    const statusDiv = document.getElementById('onboarding-status');
    const spinner = document.getElementById('onboarding-spinner');
    
    if (btn) btn.disabled = true;
    if (statusDiv) statusDiv.textContent = "Downloading AI Model... (This may take a few minutes)";
    if (spinner) spinner.style.display = 'block';
    
    const model = window.recommendedEmbeddingModel || 'all-minilm';
    
    try {
        const response = await window.electronAPI.apiRequest('POST', '/api/setup/pull', {
            model: model
        });
        
        if (response.success) {
            // Poll for completion or just wait a bit and reload?
            // Since we aren't streaming progress yet, let's set a check interval
            statusDiv.textContent = "Download started. Verifying installation...";
            
            // Poll status every 5 seconds
            const interval = setInterval(async () => {
                const statusResp = await window.electronAPI.apiRequest('GET', '/api/setup/status');
                if (statusResp.success) {
                    const status = statusResp.data.data ? statusResp.data.data : statusResp.data;
                    if (status.embedding_model_installed) {
                        clearInterval(interval);
                        completeOnboarding();
                    }
                }
            }, 5000);
            
        } else {
            statusDiv.textContent = "Error: " + (response.error || "Failed to start download");
            if (btn) btn.disabled = false;
            if (spinner) spinner.style.display = 'none';
        }
    } catch (error) {
        console.error("Download error:", error);
        if (statusDiv) statusDiv.textContent = "Error: " + error.message;
        if (btn) btn.disabled = false;
        if (spinner) spinner.style.display = 'none';
    }
};

function completeOnboarding() {
    const statusDiv = document.getElementById('onboarding-status');
    const spinner = document.getElementById('onboarding-spinner');
    const btn = document.getElementById('onboarding-start-btn');
    
    if (spinner) spinner.style.display = 'none';
    if (btn) btn.style.display = 'none';
    
    if (statusDiv) {
        statusDiv.textContent = "Setup Complete! You can now search your files.";
        statusDiv.classList.add('success');
    }
    
    setTimeout(() => {
        const modal = document.getElementById('onboarding-modal');
        if (modal) modal.classList.remove('visible');
        // Reload to refresh app state if needed
        window.location.reload();
    }, 2000);
}
