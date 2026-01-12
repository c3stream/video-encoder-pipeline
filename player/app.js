        // Player state
        let hls = null;
        let dashPlayer = null;
        let currentStreamType = 'hls';
        let settings = {
            autoQuality: true,
            verbose: false,
            lowLatency: false
        };

        // Comparison mode state
        let comparisonMode = 'normal'; // 'normal', 'sidebyside', 'ab'
        let hlsOriginal = null;
        let hlsProcessed = null;
        let dashOriginal = null;
        let dashProcessed = null;
        let currentABMode = 'original'; // 'original' or 'processed'
        let syncInterval = null;

        // Filter state
        const filterState = {
            normalize: true,
            deflicker: true,
            denoise_video: true,
            denoise_audio: true,
            photosensitivity: true,
            red_flash: true,
            color_limiter: true,
            spatial_pattern: true,
            audio_loudness: true,
            peak_limiter: true
        };
        let segmentStats = {
            video: 0,
            audio: 0,
            totalBytes: 0
        };
        let availableTiers = [];
        let currentTier = null;

        // Bitrate tracking
        const bitrateHistory = {
            video: [],
            audio: [],
            timestamps: [],
            maxPoints: 60,  // 60 seconds of data
            targetBitrate: 6000000  // Will be updated based on selected quality
        };
        let bitrateCanvas, bitrateCtx;
        let bitrateUpdateInterval = null;

        // Real-time segment download tracking
        const downloadTracker = {
            video: { bytes: 0, duration: 0, lastUpdate: 0 },
            audio: { bytes: 0, duration: 0, lastUpdate: 0 }
        };

        // DOM elements
        const video = document.getElementById('video');
        const logContainer = document.getElementById('logContainer');
        const segmentLog = document.getElementById('segmentLog');

        // Tier definitions
        const TIER_INFO = {
            1: { name: 'Tier 1', video: 'av1', audio: 'opus', prefix: 'av1_opus', score: 100 },
            2: { name: 'Tier 2', video: 'vp9', audio: 'opus', prefix: 'vp9_opus', score: 80 },
            3: { name: 'Tier 3', video: 'vp9', audio: 'aac', prefix: 'vp9_aac', score: 60 },
            4: { name: 'Tier 4', video: 'h264', audio: 'aac', prefix: 'h264_aac', score: 40 }
        };

        // Logging
        function log(message, type = 'info') {
            const entry = document.createElement('div');
            entry.className = `log-entry log-${type}`;
            const time = new Date().toLocaleTimeString();
            entry.textContent = `[${time}] ${message}`;
            logContainer.insertBefore(entry, logContainer.firstChild);
            if (logContainer.children.length > 50) {
                logContainer.removeChild(logContainer.lastChild);
            }
        }

        function logSegment(type, path, size) {
            // Always update counters regardless of verbose setting
            if (type === 'video') segmentStats.video++;
            if (type === 'audio') segmentStats.audio++;
            if (size) segmentStats.totalBytes += size;

            document.getElementById('videoSegmentCount').textContent = segmentStats.video;
            document.getElementById('audioSegmentCount').textContent = segmentStats.audio;
            document.getElementById('totalDownloaded').textContent = formatBytes(segmentStats.totalBytes);

            // Only show log entries if verbose mode or init segment
            if (!settings.verbose && type !== 'init') return;

            const entry = document.createElement('div');
            entry.className = `segment-entry ${type}`;

            const time = new Date().toLocaleTimeString('ja-JP', { hour: '2-digit', minute: '2-digit', second: '2-digit' });
            const shortPath = path.split('/').slice(-3).join('/');

            entry.innerHTML = `
                <span class="segment-time">${time}</span>
                <span class="codec-badge codec-${type === 'video' ? detectVideoCodec(path) : (type === 'audio' ? detectAudioCodec(path) : 'init')}">
                    ${type.toUpperCase()}
                </span>
                <span class="segment-path" title="${path}">${shortPath}</span>
                ${size ? `<span class="segment-size">${formatBytes(size)}</span>` : ''}
            `;

            // Remove placeholder if exists
            const placeholder = segmentLog.querySelector('div[style*="color: #666"]');
            if (placeholder) placeholder.remove();

            segmentLog.insertBefore(entry, segmentLog.firstChild);
            if (segmentLog.children.length > 30) {
                segmentLog.removeChild(segmentLog.lastChild);
            }
        }

        function detectVideoCodec(path) {
            if (path.includes('/av1/')) return 'av1';
            if (path.includes('/vp9/')) return 'vp9';
            if (path.includes('/h264/')) return 'h264';
            return 'video';
        }

        function detectAudioCodec(path) {
            if (path.includes('/opus/')) return 'opus';
            if (path.includes('/aac/')) return 'aac';
            return 'audio';
        }

        function formatBytes(bytes) {
            if (bytes === 0) return '0 B';
            const k = 1024;
            const sizes = ['B', 'KB', 'MB', 'GB'];
            const i = Math.floor(Math.log(bytes) / Math.log(k));
            return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i];
        }

        function formatBitrate(bps) {
            if (bps >= 1000000) {
                return (bps / 1000000).toFixed(2) + ' Mbps';
            } else if (bps >= 1000) {
                return (bps / 1000).toFixed(0) + ' Kbps';
            }
            return bps + ' bps';
        }

        // Bitrate Graph Functions
        function initBitrateGraph() {
            bitrateCanvas = document.getElementById('bitrateCanvas');
            if (!bitrateCanvas) return;

            bitrateCtx = bitrateCanvas.getContext('2d');

            // Set canvas resolution
            const rect = bitrateCanvas.getBoundingClientRect();
            bitrateCanvas.width = rect.width * 2;
            bitrateCanvas.height = rect.height * 2;
            bitrateCtx.scale(2, 2);

            drawBitrateGraph();
        }

        function addBitrateDataPoint(videoBps, audioBps) {
            const now = Date.now();

            bitrateHistory.video.push(videoBps);
            bitrateHistory.audio.push(audioBps);
            bitrateHistory.timestamps.push(now);

            // Keep only last N points
            while (bitrateHistory.video.length > bitrateHistory.maxPoints) {
                bitrateHistory.video.shift();
                bitrateHistory.audio.shift();
                bitrateHistory.timestamps.shift();
            }

            drawBitrateGraph();
            updateBitrateStats();
        }

        function drawBitrateGraph() {
            if (!bitrateCtx || !bitrateCanvas) return;

            const width = bitrateCanvas.width / 2;
            const height = bitrateCanvas.height / 2;
            const padding = { top: 20, right: 50, bottom: 25, left: 10 };
            const graphWidth = width - padding.left - padding.right;
            const graphHeight = height - padding.top - padding.bottom;

            // Clear canvas
            bitrateCtx.fillStyle = '#0a0a15';
            bitrateCtx.fillRect(0, 0, width, height);

            if (bitrateHistory.video.length < 2) {
                bitrateCtx.fillStyle = '#666';
                bitrateCtx.font = '12px sans-serif';
                bitrateCtx.textAlign = 'center';
                bitrateCtx.fillText('Waiting for bitrate data...', width / 2, height / 2);
                return;
            }

            // Calculate max bitrate for scale
            const allBitrates = [...bitrateHistory.video, ...bitrateHistory.audio, bitrateHistory.targetBitrate];
            const maxBitrate = Math.max(...allBitrates) * 1.2;
            const minBitrate = 0;

            // Draw grid lines
            bitrateCtx.strokeStyle = '#1a1a2e';
            bitrateCtx.lineWidth = 1;

            for (let i = 0; i <= 4; i++) {
                const y = padding.top + (graphHeight * i / 4);
                bitrateCtx.beginPath();
                bitrateCtx.moveTo(padding.left, y);
                bitrateCtx.lineTo(width - padding.right, y);
                bitrateCtx.stroke();

                // Y-axis labels
                const bitrate = maxBitrate - (maxBitrate * i / 4);
                bitrateCtx.fillStyle = '#666';
                bitrateCtx.font = '9px sans-serif';
                bitrateCtx.textAlign = 'left';
                bitrateCtx.fillText(formatBitrate(bitrate), width - padding.right + 5, y + 3);
            }

            // Draw target bitrate line (dashed)
            const targetY = padding.top + graphHeight * (1 - bitrateHistory.targetBitrate / maxBitrate);
            bitrateCtx.strokeStyle = '#ffd93d';
            bitrateCtx.lineWidth = 1;
            bitrateCtx.setLineDash([5, 5]);
            bitrateCtx.beginPath();
            bitrateCtx.moveTo(padding.left, targetY);
            bitrateCtx.lineTo(width - padding.right, targetY);
            bitrateCtx.stroke();
            bitrateCtx.setLineDash([]);

            // Draw video bitrate line
            bitrateCtx.strokeStyle = '#00d4ff';
            bitrateCtx.lineWidth = 2;
            bitrateCtx.beginPath();

            for (let i = 0; i < bitrateHistory.video.length; i++) {
                const x = padding.left + (graphWidth * i / (bitrateHistory.maxPoints - 1));
                const y = padding.top + graphHeight * (1 - bitrateHistory.video[i] / maxBitrate);

                if (i === 0) {
                    bitrateCtx.moveTo(x, y);
                } else {
                    bitrateCtx.lineTo(x, y);
                }
            }
            bitrateCtx.stroke();

            // Fill area under video bitrate
            bitrateCtx.fillStyle = 'rgba(0, 212, 255, 0.1)';
            bitrateCtx.lineTo(padding.left + graphWidth * (bitrateHistory.video.length - 1) / (bitrateHistory.maxPoints - 1), padding.top + graphHeight);
            bitrateCtx.lineTo(padding.left, padding.top + graphHeight);
            bitrateCtx.closePath();
            bitrateCtx.fill();

            // Draw audio bitrate line
            bitrateCtx.strokeStyle = '#00ff88';
            bitrateCtx.lineWidth = 1.5;
            bitrateCtx.beginPath();

            for (let i = 0; i < bitrateHistory.audio.length; i++) {
                const x = padding.left + (graphWidth * i / (bitrateHistory.maxPoints - 1));
                const y = padding.top + graphHeight * (1 - bitrateHistory.audio[i] / maxBitrate);

                if (i === 0) {
                    bitrateCtx.moveTo(x, y);
                } else {
                    bitrateCtx.lineTo(x, y);
                }
            }
            bitrateCtx.stroke();

            // Time labels
            bitrateCtx.fillStyle = '#666';
            bitrateCtx.font = '9px sans-serif';
            bitrateCtx.textAlign = 'center';
            bitrateCtx.fillText('60s ago', padding.left, height - 5);
            bitrateCtx.fillText('now', width - padding.right, height - 5);
        }

        function updateBitrateStats() {
            if (bitrateHistory.video.length === 0) return;

            const videoBitrates = bitrateHistory.video.filter(b => b > 0);
            if (videoBitrates.length === 0) return;

            const current = videoBitrates[videoBitrates.length - 1];
            const avg = videoBitrates.reduce((a, b) => a + b, 0) / videoBitrates.length;
            const peak = Math.max(...videoBitrates);
            const min = Math.min(...videoBitrates);

            document.getElementById('bitrateCurrentStat').textContent = formatBitrate(current);
            document.getElementById('bitrateAvgStat').textContent = formatBitrate(avg);
            document.getElementById('bitratePeakStat').textContent = formatBitrate(peak);
            document.getElementById('bitrateMinStat').textContent = formatBitrate(min);
        }

        function startBitrateTracking() {
            if (bitrateUpdateInterval) clearInterval(bitrateUpdateInterval);

            bitrateHistory.video = [];
            bitrateHistory.audio = [];
            bitrateHistory.timestamps = [];

            // Reset download tracker
            downloadTracker.video = { bytes: 0, duration: 0, lastUpdate: Date.now() };
            downloadTracker.audio = { bytes: 0, duration: 0, lastUpdate: Date.now() };

            initBitrateGraph();

            bitrateUpdateInterval = setInterval(() => {
                const now = Date.now();
                let videoBps = 0;
                let audioBps = 0;

                // Calculate bitrate from download tracker
                if (currentStreamType === 'hls' && hls) {
                    try {
                        // Use HLS.js level bitrate as reference
                        const currentLevel = hls.currentLevel >= 0 ? hls.currentLevel : hls.loadLevel;
                        const level = hls.levels && hls.levels[currentLevel];
                        if (level) {
                            videoBps = level.bitrate || level.attrs?.BANDWIDTH || 0;
                            bitrateHistory.targetBitrate = videoBps;
                        }
                        audioBps = 128000;
                    } catch (e) {
                        console.error('HLS bitrate error:', e);
                    }
                } else if (currentStreamType === 'dash' && dashPlayer) {
                    try {
                        // Method 1: Use download tracker (actual throughput)
                        const videoElapsed = (now - downloadTracker.video.lastUpdate) / 1000;
                        if (downloadTracker.video.bytes > 0 && videoElapsed > 0 && videoElapsed < 5) {
                            videoBps = (downloadTracker.video.bytes * 8) / downloadTracker.video.duration;
                        }

                        // Method 2: Use current quality's declared bandwidth
                        if (videoBps === 0 || !isFinite(videoBps)) {
                            const currentQuality = dashPlayer.getQualityFor ?
                                dashPlayer.getQualityFor('video') : -1;
                            if (currentQuality >= 0) {
                                const representations = dashPlayer.getRepresentationsByType ?
                                    dashPlayer.getRepresentationsByType('video') : [];
                                if (representations && representations[currentQuality]) {
                                    videoBps = representations[currentQuality].bandwidth || 0;
                                    bitrateHistory.targetBitrate = videoBps;
                                }
                            }
                        }

                        // Method 3: Get from streaming info
                        if (videoBps === 0 || !isFinite(videoBps)) {
                            const streamInfo = dashPlayer.getActiveStream ? dashPlayer.getActiveStream() : null;
                            if (streamInfo) {
                                const streamProcessor = streamInfo.getProcessors ? streamInfo.getProcessors() : [];
                                streamProcessor.forEach(p => {
                                    if (p.getType && p.getType() === 'video') {
                                        const rep = p.getRepresentation ? p.getRepresentation() : null;
                                        if (rep && rep.bandwidth) {
                                            videoBps = rep.bandwidth;
                                        }
                                    }
                                });
                            }
                        }

                        // Get audio bitrate from representation
                        const audioQuality = dashPlayer.getQualityFor ?
                            dashPlayer.getQualityFor('audio') : -1;
                        if (audioQuality >= 0) {
                            const audioReps = dashPlayer.getRepresentationsByType ?
                                dashPlayer.getRepresentationsByType('audio') : [];
                            if (audioReps && audioReps[audioQuality]) {
                                audioBps = audioReps[audioQuality].bandwidth || 128000;
                            }
                        }
                        if (audioBps === 0) audioBps = 128000;

                    } catch (e) {
                        console.error('DASH bitrate error:', e);
                    }
                }

                // Only add valid data points
                if (videoBps > 0 && isFinite(videoBps) && videoBps < 100000000) {
                    addBitrateDataPoint(videoBps, audioBps);
                }
            }, 1000);
        }

        // Track actual segment downloads
        function trackSegmentDownload(type, bytes, duration) {
            if (type === 'video') {
                downloadTracker.video.bytes = bytes;
                downloadTracker.video.duration = duration > 0 ? duration : 4; // default 4s segment
                downloadTracker.video.lastUpdate = Date.now();
            } else if (type === 'audio') {
                downloadTracker.audio.bytes = bytes;
                downloadTracker.audio.duration = duration > 0 ? duration : 4;
                downloadTracker.audio.lastUpdate = Date.now();
            }
        }

        function stopBitrateTracking() {
            if (bitrateUpdateInterval) {
                clearInterval(bitrateUpdateInterval);
                bitrateUpdateInterval = null;
            }
        }

        // Stream type selection
        function selectStreamType(type) {
            currentStreamType = type;
            document.querySelectorAll('.stream-tab').forEach(tab => {
                tab.classList.toggle('active', tab.dataset.type === type);
            });

            // Update URL placeholder
            const urlInput = document.getElementById('manifestUrl');
            if (type === 'hls') {
                urlInput.placeholder = 'http://localhost:8080/output/hls/master.m3u8';
            } else {
                urlInput.placeholder = 'http://localhost:8080/output/dash/manifest.mpd';
            }

            // Show/hide tier panel for HLS
            document.getElementById('tierPanel').querySelector('h3').textContent =
                type === 'hls' ? '🎯 Tier Selection (HLS)' : '🎯 Codec Info (DASH)';

            log(`Selected ${type.toUpperCase()} stream type`, 'info');
        }

        // Quick load functions
        function quickLoad(type) {
            selectStreamType(type);
            const baseUrl = 'http://localhost:8080';
            const url = type === 'hls'
                ? `${baseUrl}/output/hls/master.m3u8`
                : `${baseUrl}/output/dash/manifest.mpd`;
            document.getElementById('manifestUrl').value = url;
            loadStream();
        }

        function copyCurrentUrl() {
            const url = document.getElementById('manifestUrl').value;
            if (url) {
                navigator.clipboard.writeText(url);
                log('URL copied to clipboard', 'success');
            }
        }

        // Settings toggle
        function toggleSetting(setting) {
            settings[setting] = !settings[setting];
            document.getElementById(`toggle${setting.charAt(0).toUpperCase() + setting.slice(1)}`)
                .classList.toggle('active', settings[setting]);

            if (setting === 'autoQuality') {
                if (hls) {
                    hls.currentLevel = settings.autoQuality ? -1 : hls.currentLevel;
                }
                if (dashPlayer) {
                    dashPlayer.updateSettings({
                        streaming: { abr: { autoSwitchBitrate: { video: settings.autoQuality } } }
                    });
                }
            }

            log(`${setting} ${settings[setting] ? 'enabled' : 'disabled'}`, 'info');
        }

        // Stop stream
        function stopStream() {
            // Stop bitrate tracking
            stopBitrateTracking();

            if (hls) {
                hls.destroy();
                hls = null;
            }
            if (dashPlayer) {
                dashPlayer.reset();
                dashPlayer = null;
            }
            video.src = '';

            // Reset UI
            resetTierCards();
            document.getElementById('qualityLevels').innerHTML =
                '<button class="quality-btn" disabled>Load stream first</button>';
            document.getElementById('videoCodec').textContent = '-';
            document.getElementById('audioCodec').textContent = '-';
            document.getElementById('currentResolution').textContent = '-';
            document.getElementById('currentBitrate').textContent = '-';

            segmentStats = { video: 0, audio: 0, totalBytes: 0 };
            document.getElementById('videoSegmentCount').textContent = '0';
            document.getElementById('audioSegmentCount').textContent = '0';
            document.getElementById('totalDownloaded').textContent = '0 KB';

            // Reset bitrate stats
            document.getElementById('bitrateCurrentStat').textContent = '-';
            document.getElementById('bitrateAvgStat').textContent = '-';
            document.getElementById('bitratePeakStat').textContent = '-';
            document.getElementById('bitrateMinStat').textContent = '-';
            bitrateHistory.video = [];
            bitrateHistory.audio = [];
            bitrateHistory.timestamps = [];
            initBitrateGraph();

            segmentLog.innerHTML = '<div style="color: #666; text-align: center; padding: 20px;">Segments will appear here...</div>';
            document.getElementById('sharedInfo').innerHTML =
                '<div style="color: #666; text-align: center; padding: 10px;">Load a stream to see shared segment info</div>';

            log('Stream stopped', 'info');
        }

        // Reset tier cards
        function resetTierCards() {
            document.querySelectorAll('.tier-card').forEach(card => {
                card.classList.remove('active');
                card.classList.add('disabled');
            });
            availableTiers = [];
            currentTier = null;
        }

        // Load stream
        function loadStream() {
            stopStream();

            const url = document.getElementById('manifestUrl').value.trim();
            if (!url) {
                log('Please enter a manifest URL', 'error');
                return;
            }

            // Auto-detect type from URL
            if (url.endsWith('.m3u8')) {
                selectStreamType('hls');
            } else if (url.endsWith('.mpd')) {
                selectStreamType('dash');
            }

            log(`Loading ${currentStreamType.toUpperCase()} stream: ${url}`, 'info');

            // Start bitrate tracking
            startBitrateTracking();

            if (currentStreamType === 'hls') {
                loadHLS(url);
            } else {
                loadDASH(url);
            }
        }

        // HLS Loading
        function loadHLS(url) {
            if (!Hls.isSupported()) {
                if (video.canPlayType('application/vnd.apple.mpegurl')) {
                    video.src = url;
                    log('Using native HLS support (Safari)', 'info');
                } else {
                    log('HLS not supported in this browser', 'error');
                    return;
                }
                return;
            }

            hls = new Hls({
                debug: settings.verbose,
                enableWorker: true,
                lowLatencyMode: settings.lowLatency,
                xhrSetup: function(xhr, url) {
                    // Track segment loading
                    xhr.addEventListener('load', function() {
                        if (url.includes('.m4s') || url.includes('.mp4')) {
                            const size = parseInt(xhr.getResponseHeader('content-length')) || xhr.response?.byteLength || 0;
                            const isInit = url.includes('init');
                            const type = isInit ? 'init' :
                                         (url.includes('/video/') ? 'video' :
                                          url.includes('/audio/') ? 'audio' : 'init');
                            logSegment(type, url, size);

                            // Track for bitrate calculation
                            if (!isInit && size > 0) {
                                trackSegmentDownload(type, size, 4); // Assume 4s segments
                            }
                        }
                    });
                }
            });

            hls.on(Hls.Events.MANIFEST_PARSED, (event, data) => {
                log(`Manifest parsed: ${data.levels.length} quality levels`, 'success');
                parseHLSManifest(data);
                video.play().catch(e => log('Autoplay blocked: ' + e.message, 'warn'));
            });

            hls.on(Hls.Events.LEVEL_SWITCHED, (event, data) => {
                const level = hls.levels[data.level];
                updateCurrentStats(level);
                log(`Quality switched to ${level?.height}p`, 'info');
            });

            hls.on(Hls.Events.AUDIO_TRACK_SWITCHED, (event, data) => {
                log(`Audio track switched: ${data.id}`, 'info');
            });

            hls.on(Hls.Events.ERROR, (event, data) => {
                if (data.fatal) {
                    log(`Fatal error: ${data.type} - ${data.details}`, 'error');
                    if (data.type === Hls.ErrorTypes.NETWORK_ERROR) {
                        log('Attempting to recover from network error...', 'warn');
                        hls.startLoad();
                    } else if (data.type === Hls.ErrorTypes.MEDIA_ERROR) {
                        log('Attempting to recover from media error...', 'warn');
                        hls.recoverMediaError();
                    }
                } else if (settings.verbose) {
                    log(`Non-fatal error: ${data.details}`, 'warn');
                }
            });

            hls.loadSource(url);
            hls.attachMedia(video);
        }

        // Parse HLS manifest and detect tiers
        function parseHLSManifest(data) {
            const levels = data.levels;
            const audioTracks = hls.audioTracks || [];

            // Detect available tiers from variant streams
            availableTiers = [];
            const tierMap = {};

            levels.forEach((level, index) => {
                const codecs = level.attrs?.CODECS || level.codecSet || '';
                let videoCodec = 'unknown';
                let audioCodec = 'unknown';

                // Detect video codec
                if (codecs.includes('av01') || codecs.includes('av1')) videoCodec = 'av1';
                else if (codecs.includes('vp09') || codecs.includes('vp9')) videoCodec = 'vp9';
                else if (codecs.includes('avc1') || codecs.includes('h264')) videoCodec = 'h264';

                // Detect audio codec
                if (codecs.includes('opus')) audioCodec = 'opus';
                else if (codecs.includes('mp4a')) audioCodec = 'aac';

                // Map to tier
                for (const [tierNum, tierInfo] of Object.entries(TIER_INFO)) {
                    if (tierInfo.video === videoCodec && tierInfo.audio === audioCodec) {
                        if (!tierMap[tierNum]) {
                            tierMap[tierNum] = [];
                            availableTiers.push(parseInt(tierNum));
                        }
                        tierMap[tierNum].push({ index, level, videoCodec, audioCodec });
                    }
                }
            });

            // Update tier cards
            updateTierCards(tierMap);

            // Update quality buttons
            updateQualityButtons(levels);

            // Update shared segments info
            updateSharedInfo(tierMap);
        }

        // Update tier card UI
        function updateTierCards(tierMap) {
            document.querySelectorAll('.tier-card').forEach(card => {
                const tier = parseInt(card.dataset.tier);
                if (availableTiers.includes(tier)) {
                    card.classList.remove('disabled');
                    card.onclick = () => selectTier(tier, tierMap[tier]);
                } else {
                    card.classList.add('disabled');
                    card.onclick = null;
                }
                card.classList.remove('active');
            });

            // Auto-select first available tier
            if (availableTiers.length > 0) {
                const firstTier = Math.min(...availableTiers);
                selectTier(firstTier, tierMap[firstTier]);
            }
        }

        // Select tier
        function selectTier(tier, variants) {
            currentTier = tier;
            const tierInfo = TIER_INFO[tier];

            // Update UI
            document.querySelectorAll('.tier-card').forEach(card => {
                card.classList.toggle('active', parseInt(card.dataset.tier) === tier);
            });

            // Update codec display
            document.getElementById('videoCodec').textContent = tierInfo.video.toUpperCase();
            document.getElementById('audioCodec').textContent = tierInfo.audio.toUpperCase();

            // Filter quality levels to this tier's variants
            if (hls && variants) {
                const levelIndices = variants.map(v => v.index);

                // Update quality buttons to show only this tier's levels
                const container = document.getElementById('qualityLevels');
                container.innerHTML = '';

                // Auto button
                const autoBtn = document.createElement('button');
                autoBtn.className = 'quality-btn' + (settings.autoQuality ? ' active' : '');
                autoBtn.textContent = 'Auto';
                autoBtn.onclick = () => {
                    settings.autoQuality = true;
                    hls.currentLevel = -1;
                    updateQualityButtonState(-1);
                    log('Switched to auto quality', 'info');
                };
                container.appendChild(autoBtn);

                variants.forEach(({ index, level }) => {
                    const btn = document.createElement('button');
                    btn.className = 'quality-btn';
                    btn.textContent = `${level.height}p`;
                    btn.dataset.level = index;
                    btn.onclick = () => {
                        settings.autoQuality = false;
                        hls.currentLevel = index;
                        updateQualityButtonState(index);
                        log(`Selected ${level.height}p quality`, 'info');
                    };
                    container.appendChild(btn);
                });
            }

            log(`Selected ${tierInfo.name}: ${tierInfo.video.toUpperCase()} + ${tierInfo.audio.toUpperCase()}`, 'success');
        }

        // Select tier for DASH (codec preference)
        function selectDashTier(tier, tierInfo) {
            currentTier = tier;

            // Update UI
            document.querySelectorAll('.tier-card').forEach(card => {
                card.classList.toggle('active', parseInt(card.dataset.tier) === tier);
            });

            // Update codec display
            document.getElementById('videoCodec').textContent = tierInfo.video.toUpperCase();
            document.getElementById('audioCodec').textContent = tierInfo.audio.toUpperCase();

            // Note: DASH player automatically selects codec based on browser support
            // We can't force a specific codec in dash.js like we can with HLS tiers
            // This is informational - showing what codec tier the user prefers

            log(`Selected ${tierInfo.name}: ${tierInfo.video.toUpperCase()} + ${tierInfo.audio.toUpperCase()}`, 'success');
        }

        // Update quality buttons for all levels
        function updateQualityButtons(levels) {
            const container = document.getElementById('qualityLevels');
            container.innerHTML = '';

            const autoBtn = document.createElement('button');
            autoBtn.className = 'quality-btn active';
            autoBtn.textContent = 'Auto';
            autoBtn.onclick = () => {
                settings.autoQuality = true;
                if (hls) hls.currentLevel = -1;
                updateQualityButtonState(-1);
                log('Switched to auto quality', 'info');
            };
            container.appendChild(autoBtn);

            levels.forEach((level, index) => {
                const btn = document.createElement('button');
                btn.className = 'quality-btn';
                btn.textContent = `${level.height}p`;
                btn.dataset.level = index;
                btn.onclick = () => {
                    settings.autoQuality = false;
                    if (hls) hls.currentLevel = index;
                    updateQualityButtonState(index);
                    log(`Selected ${level.height}p quality`, 'info');
                };
                container.appendChild(btn);
            });
        }

        function updateQualityButtonState(activeLevel) {
            document.querySelectorAll('.quality-btn').forEach(btn => {
                const level = parseInt(btn.dataset.level);
                btn.classList.toggle('active',
                    (activeLevel === -1 && btn.textContent === 'Auto') ||
                    level === activeLevel
                );
            });
        }

        // Update shared segments info
        function updateSharedInfo(tierMap) {
            const container = document.getElementById('sharedInfo');

            // Analyze shared codecs
            const videoCodecs = new Set();
            const audioCodecs = new Set();

            for (const tier of availableTiers) {
                const info = TIER_INFO[tier];
                videoCodecs.add(info.video);
                audioCodecs.add(info.audio);
            }

            let html = '';

            // Video segments sharing
            html += '<div class="manifest-section">';
            html += '<div class="manifest-title">Video Segments</div>';
            videoCodecs.forEach(codec => {
                const tiers = availableTiers.filter(t => TIER_INFO[t].video === codec);
                if (tiers.length > 1) {
                    html += `<div class="manifest-item">
                        <span><span class="codec-badge codec-${codec}">${codec.toUpperCase()}</span></span>
                        <span class="shared-indicator">Shared by Tier ${tiers.join(', ')}</span>
                    </div>`;
                } else if (tiers.length === 1) {
                    html += `<div class="manifest-item">
                        <span><span class="codec-badge codec-${codec}">${codec.toUpperCase()}</span></span>
                        <span>Tier ${tiers[0]} only</span>
                    </div>`;
                }
            });
            html += '</div>';

            // Audio segments sharing
            html += '<div class="manifest-section">';
            html += '<div class="manifest-title">Audio Segments</div>';
            audioCodecs.forEach(codec => {
                const tiers = availableTiers.filter(t => TIER_INFO[t].audio === codec);
                if (tiers.length > 1) {
                    html += `<div class="manifest-item">
                        <span><span class="codec-badge codec-${codec}">${codec.toUpperCase()}</span></span>
                        <span class="shared-indicator">Shared by Tier ${tiers.join(', ')}</span>
                    </div>`;
                } else if (tiers.length === 1) {
                    html += `<div class="manifest-item">
                        <span><span class="codec-badge codec-${codec}">${codec.toUpperCase()}</span></span>
                        <span>Tier ${tiers[0]} only</span>
                    </div>`;
                }
            });
            html += '</div>';

            // Storage efficiency
            const encodings = videoCodecs.size * 4 + audioCodecs.size; // 4 resolutions per video codec
            const tierCount = availableTiers.length * 4 + availableTiers.length; // if separate
            const savings = Math.round((1 - encodings / tierCount) * 100);

            if (savings > 0) {
                html += `<div class="manifest-section">
                    <div class="manifest-title">Storage Efficiency</div>
                    <div class="manifest-item">
                        <span>Actual Encodings</span>
                        <span>${encodings}</span>
                    </div>
                    <div class="manifest-item">
                        <span>Without Sharing</span>
                        <span>${tierCount}</span>
                    </div>
                    <div class="manifest-item">
                        <span>Savings</span>
                        <span class="shared-indicator">${savings}%</span>
                    </div>
                </div>`;
            }

            container.innerHTML = html;
        }

        // DASH Loading
        function loadDASH(url) {
            dashPlayer = dashjs.MediaPlayer().create();

            dashPlayer.updateSettings({
                debug: { logLevel: settings.verbose ? dashjs.Debug.LOG_LEVEL_DEBUG : dashjs.Debug.LOG_LEVEL_WARNING },
                streaming: {
                    lowLatencyEnabled: settings.lowLatency,
                    abr: { autoSwitchBitrate: { video: settings.autoQuality } }
                }
            });

            dashPlayer.on(dashjs.MediaPlayer.events.MANIFEST_LOADED, (e) => {
                log('DASH manifest loaded', 'success');
                parseDASHManifest(e.data);
            });

            dashPlayer.on(dashjs.MediaPlayer.events.STREAM_INITIALIZED, (e) => {
                try {
                    const bitrateInfo = dashPlayer.getRepresentationsByType ?
                        dashPlayer.getRepresentationsByType('video') :
                        (dashPlayer.getBitrateInfoListFor ? dashPlayer.getBitrateInfoListFor('video') : []);
                    if (bitrateInfo && bitrateInfo.length > 0) {
                        updateDashQualityLevels(bitrateInfo);
                    }
                    log(`Stream initialized: ${bitrateInfo?.length || 0} quality levels`, 'success');
                } catch (err) {
                    log(`Stream initialized`, 'success');
                }
            });

            dashPlayer.on(dashjs.MediaPlayer.events.QUALITY_CHANGE_RENDERED, (e) => {
                if (e.mediaType === 'video') {
                    try {
                        const bitrateInfo = dashPlayer.getRepresentationsByType ?
                            dashPlayer.getRepresentationsByType('video') :
                            (dashPlayer.getBitrateInfoListFor ? dashPlayer.getBitrateInfoListFor('video') : []);
                        const currentLevel = bitrateInfo[e.newQuality];
                        if (currentLevel) {
                            updateCurrentStats(currentLevel);
                        }
                    } catch (err) {}
                    log(`Quality changed to level ${e.newQuality}`, 'info');
                }
            });

            dashPlayer.on(dashjs.MediaPlayer.events.FRAGMENT_LOADING_COMPLETED, (e) => {
                const req = e.request;
                if (req) {
                    const mediaType = req.mediaType;
                    const isInit = req.type === 'InitializationSegment';
                    const type = isInit ? 'init' :
                                 (mediaType === 'video' ? 'video' :
                                  mediaType === 'audio' ? 'audio' : 'init');
                    const size = req.bytesTotal || e.response?.byteLength || 0;
                    const duration = req.duration || 4;

                    // Log segment
                    logSegment(type, req.url || '', size);

                    // Track download for bitrate calculation
                    if (!isInit && size > 0) {
                        trackSegmentDownload(type, size, duration);
                    }

                    if (settings.verbose) {
                        console.log(`DASH Segment: ${type}, size=${size}, duration=${duration}`);
                    }
                }
            });

            dashPlayer.on(dashjs.MediaPlayer.events.ERROR, (e) => {
                log(`DASH error: ${e.error?.message || JSON.stringify(e)}`, 'error');
            });

            dashPlayer.initialize(video, url, true);
        }

        // Parse DASH manifest
        function parseDASHManifest(manifest) {
            if (!manifest) {
                log('DASH manifest is empty', 'error');
                return;
            }

            const periods = manifest.Period_asArray || (Array.isArray(manifest.Period) ? manifest.Period : (manifest.Period ? [manifest.Period] : []));
            const adaptationSets = [];

            periods.forEach(period => {
                if (!period) return;
                const sets = period.AdaptationSet_asArray || (Array.isArray(period.AdaptationSet) ? period.AdaptationSet : (period.AdaptationSet ? [period.AdaptationSet] : []));
                adaptationSets.push(...sets.filter(s => s));
            });

            // Analyze codecs
            const videoCodecs = new Set();
            const audioCodecs = new Set();

            adaptationSets.forEach(set => {
                if (!set) return;
                const mimeType = set.mimeType || set['@mimeType'] || '';
                const codecs = set.codecs || set['@codecs'] || '';

                if (mimeType.includes('video')) {
                    if (codecs.includes('av01') || codecs.includes('av1')) videoCodecs.add('av1');
                    else if (codecs.includes('vp09') || codecs.includes('vp9')) videoCodecs.add('vp9');
                    else if (codecs.includes('avc1')) videoCodecs.add('h264');
                } else if (mimeType.includes('audio')) {
                    if (codecs.includes('opus')) audioCodecs.add('opus');
                    else if (codecs.includes('mp4a')) audioCodecs.add('aac');
                }
            });

            // Update codec display
            document.getElementById('videoCodec').textContent = [...videoCodecs].map(c => c.toUpperCase()).join(', ');
            document.getElementById('audioCodec').textContent = [...audioCodecs].map(c => c.toUpperCase()).join(', ');

            // Update tier cards for DASH (show available codecs)
            document.querySelectorAll('.tier-card').forEach(card => {
                const tier = parseInt(card.dataset.tier);
                const info = TIER_INFO[tier];
                const available = videoCodecs.has(info.video) && audioCodecs.has(info.audio);

                if (available) {
                    card.classList.remove('disabled');
                    availableTiers.push(tier);
                    // Add click handler for DASH tier selection
                    card.onclick = () => selectDashTier(tier, info);
                } else {
                    card.classList.add('disabled');
                    card.onclick = null;
                }
            });

            // Update shared info for DASH
            updateDashSharedInfo(videoCodecs, audioCodecs);
        }

        function updateDashSharedInfo(videoCodecs, audioCodecs) {
            const container = document.getElementById('sharedInfo');

            let html = '<div class="manifest-section">';
            html += '<div class="manifest-title">Available Codecs (DASH)</div>';

            videoCodecs.forEach(codec => {
                html += `<div class="manifest-item">
                    <span><span class="codec-badge codec-${codec}">${codec.toUpperCase()}</span> Video</span>
                    <span>Available</span>
                </div>`;
            });

            audioCodecs.forEach(codec => {
                html += `<div class="manifest-item">
                    <span><span class="codec-badge codec-${codec}">${codec.toUpperCase()}</span> Audio</span>
                    <span>Available</span>
                </div>`;
            });

            html += '</div>';

            html += '<div class="manifest-section">';
            html += '<div class="manifest-title">Note</div>';
            html += '<div style="color: #888; font-size: 10px;">DASH player selects codec based on browser support. All resolutions share the same encoded segments.</div>';
            html += '</div>';

            container.innerHTML = html;
        }

        function updateDashQualityLevels(levels) {
            const container = document.getElementById('qualityLevels');
            container.innerHTML = '';

            const autoBtn = document.createElement('button');
            autoBtn.className = 'quality-btn' + (settings.autoQuality ? ' active' : '');
            autoBtn.textContent = 'Auto';
            autoBtn.onclick = () => {
                settings.autoQuality = true;
                dashPlayer.updateSettings({ streaming: { abr: { autoSwitchBitrate: { video: true } } } });
                updateQualityButtonState(-1);
                log('Switched to auto quality', 'info');
            };
            container.appendChild(autoBtn);

            levels.forEach((level, index) => {
                const btn = document.createElement('button');
                btn.className = 'quality-btn';
                btn.textContent = `${level.height}p`;
                btn.dataset.level = index;
                btn.onclick = () => {
                    settings.autoQuality = false;
                    dashPlayer.updateSettings({ streaming: { abr: { autoSwitchBitrate: { video: false } } } });
                    dashPlayer.setQualityFor('video', index);
                    updateQualityButtonState(index);
                    log(`Selected ${level.height}p quality`, 'info');
                };
                container.appendChild(btn);
            });
        }

        // Update current stats
        function updateCurrentStats(level) {
            if (!level) return;

            document.getElementById('currentResolution').textContent =
                `${level.width || level.attrs?.RESOLUTION?.split('x')[0] || '-'}x${level.height}`;
            document.getElementById('currentBitrate').textContent =
                `${Math.round((level.bitrate || level.bandwidth || 0) / 1000)} kbps`;
        }

        // Update buffer and dropped frames periodically
        setInterval(() => {
            if (video.buffered.length > 0) {
                const buffered = video.buffered.end(video.buffered.length - 1) - video.currentTime;
                document.getElementById('bufferLength').textContent = `${buffered.toFixed(1)}s`;
            }

            if (video.getVideoPlaybackQuality) {
                const quality = video.getVideoPlaybackQuality();
                document.getElementById('droppedFrames').textContent = quality.droppedVideoFrames;
            }
        }, 1000);

        // Keyboard shortcuts
        document.addEventListener('keydown', (e) => {
            if (e.target.tagName === 'INPUT') return;

            switch(e.key) {
                case ' ':
                    e.preventDefault();
                    video.paused ? video.play() : video.pause();
                    break;
                case 'f':
                    if (document.fullscreenElement) {
                        document.exitFullscreen();
                    } else {
                        video.requestFullscreen();
                    }
                    break;
                case 'm':
                    video.muted = !video.muted;
                    break;
                case '1': case '2': case '3': case '4':
                    const tier = parseInt(e.key);
                    if (availableTiers.includes(tier)) {
                        document.querySelector(`.tier-card[data-tier="${tier}"]`)?.click();
                    }
                    break;
                // Comparison mode shortcuts
                case 'a':
                case 'A':
                    if (comparisonMode === 'ab') {
                        switchABMode('original');
                    }
                    break;
                case 'b':
                case 'B':
                    if (comparisonMode === 'ab') {
                        switchABMode('processed');
                    }
                    break;
                case 'c':
                case 'C':
                    // Cycle through comparison modes
                    if (comparisonMode === 'normal') {
                        setComparisonMode('sidebyside');
                    } else if (comparisonMode === 'sidebyside') {
                        setComparisonMode('ab');
                    } else {
                        setComparisonMode('normal');
                    }
                    break;
                case 'Tab':
                    // Quick toggle between A and B in A/B mode
                    if (comparisonMode === 'ab') {
                        e.preventDefault();
                        switchABMode(currentABMode === 'original' ? 'processed' : 'original');
                    }
                    break;
            }
        });

        // ========================================
        // Comparison Mode Functions
        // ========================================

        function setComparisonMode(mode) {
            comparisonMode = mode;

            // Update UI tabs
            document.querySelectorAll('.comparison-mode-tab').forEach(tab => {
                tab.classList.toggle('active', tab.dataset.mode === mode);
            });

            // Show/hide comparison URL inputs
            const urlInputs = document.getElementById('comparisonUrlInputs');
            urlInputs.style.display = (mode !== 'normal') ? 'block' : 'none';

            // Switch player modes
            const normalPlayer = document.getElementById('normalPlayerMode');
            const comparisonContainer = document.getElementById('comparisonContainer');
            const abToggle = document.getElementById('abToggleContainer');
            const syncStatus = document.getElementById('syncStatus');

            if (mode === 'normal') {
                normalPlayer.style.display = 'block';
                comparisonContainer.classList.remove('active');
                abToggle.style.display = 'none';
                syncStatus.style.display = 'none';
                stopComparisonStreams();
            } else if (mode === 'sidebyside') {
                normalPlayer.style.display = 'none';
                comparisonContainer.classList.add('active');
                comparisonContainer.classList.remove('single');
                abToggle.style.display = 'none';
                syncStatus.style.display = 'flex';
            } else if (mode === 'ab') {
                normalPlayer.style.display = 'none';
                comparisonContainer.classList.add('active');
                comparisonContainer.classList.add('single');
                abToggle.style.display = 'flex';
                syncStatus.style.display = 'flex';
                // In A/B mode, show only one video at a time
                updateABVisibility();
            }

            log(`比較モード: ${mode === 'normal' ? '通常' : mode === 'sidebyside' ? '並列比較' : 'A/B切替'}`, 'info');
        }

        function switchABMode(mode) {
            currentABMode = mode;

            // Update buttons
            document.querySelectorAll('.ab-toggle-btn').forEach(btn => {
                btn.classList.toggle('active', btn.classList.contains(mode));
            });

            updateABVisibility();
            log(`A/B切替: ${mode === 'original' ? 'オリジナル' : '処理済み'}`, 'info');
        }

        function updateABVisibility() {
            const wrappers = document.querySelectorAll('.comparison-video-wrapper');
            if (comparisonMode === 'ab' && wrappers.length >= 2) {
                wrappers[0].style.display = currentABMode === 'original' ? 'block' : 'none';
                wrappers[1].style.display = currentABMode === 'processed' ? 'block' : 'none';
            } else {
                wrappers.forEach(w => w.style.display = 'block');
            }
        }

        function loadComparisonStreams() {
            const originalUrl = document.getElementById('originalUrl').value.trim();
            const processedUrl = document.getElementById('processedUrl').value.trim();

            if (!originalUrl || !processedUrl) {
                log('両方のURLを入力してください', 'error');
                return;
            }

            // Ensure comparison mode is active
            if (comparisonMode === 'normal') {
                setComparisonMode('sidebyside');
            }

            stopComparisonStreams();

            log(`比較ストリーム読み込み中...`, 'info');

            const videoOriginal = document.getElementById('videoOriginal');
            const videoProcessed = document.getElementById('videoProcessed');

            // Detect stream type from URL
            const isHLS = originalUrl.endsWith('.m3u8');

            if (isHLS) {
                loadComparisonHLS(originalUrl, processedUrl, videoOriginal, videoProcessed);
            } else {
                loadComparisonDASH(originalUrl, processedUrl, videoOriginal, videoProcessed);
            }

            // Start sync
            startVideoSync(videoOriginal, videoProcessed);
        }

        function loadComparisonHLS(originalUrl, processedUrl, videoOriginal, videoProcessed) {
            if (!Hls.isSupported()) {
                log('HLS not supported', 'error');
                return;
            }

            let originalReady = false;
            let processedReady = false;

            const tryAutoPlay = () => {
                if (originalReady && processedReady) {
                    // Auto-play both with muted (browser autoplay policy requires muted)
                    videoOriginal.muted = true;
                    videoProcessed.muted = true;

                    Promise.all([
                        videoOriginal.play(),
                        videoProcessed.play()
                    ]).then(() => {
                        log('再生開始（ミュート）- 音声を有効にするにはコントロールからミュート解除', 'info');
                    }).catch(err => {
                        log(`自動再生に失敗: ${err.message} - 再生ボタンをクリックしてください`, 'warning');
                    });
                }
            };

            // HLS config to handle buffer issues
            const hlsConfig = {
                debug: settings.verbose,
                enableWorker: true,
                lowLatencyMode: false,
                // Handle buffer gaps gracefully
                maxBufferHole: 0.5,
                maxSeekHole: 2,
                // Reduce aggressive seeking
                nudgeOffset: 0.1,
                nudgeMaxRetry: 5
            };

            // Load original
            hlsOriginal = new Hls(hlsConfig);
            hlsOriginal.on(Hls.Events.MANIFEST_LOADED, (event, data) => {
                log(`オリジナル マニフェスト読み込み: levels=${data.levels?.length || 0}`, 'info');
            });
            hlsOriginal.on(Hls.Events.MANIFEST_PARSED, (event, data) => {
                log(`オリジナルストリーム読み込み完了: levels=${data.levels?.length || 0}`, 'success');
                originalReady = true;
                tryAutoPlay();
            });
            hlsOriginal.on(Hls.Events.LEVEL_LOADED, (event, data) => {
                log(`オリジナル レベル読み込み完了: duration=${data.details?.totalduration?.toFixed(1)}s`, 'info');
            });
            hlsOriginal.on(Hls.Events.ERROR, (event, data) => {
                // Only log fatal errors or non-recoverable issues
                if (data.fatal) {
                    log(`HLS Original 致命的エラー: ${data.details}`, 'error');
                    // Try to recover
                    if (data.type === Hls.ErrorTypes.MEDIA_ERROR) {
                        hlsOriginal.recoverMediaError();
                    }
                }
                // Silently handle recoverable buffer errors
            });
            hlsOriginal.attachMedia(videoOriginal);
            hlsOriginal.loadSource(originalUrl);

            // Load processed
            hlsProcessed = new Hls(hlsConfig);
            hlsProcessed.on(Hls.Events.MANIFEST_LOADED, (event, data) => {
                log(`処理済み マニフェスト読み込み: levels=${data.levels?.length || 0}`, 'info');
            });
            hlsProcessed.on(Hls.Events.MANIFEST_PARSED, (event, data) => {
                log(`処理済みストリーム読み込み完了: levels=${data.levels?.length || 0}`, 'success');
                processedReady = true;
                tryAutoPlay();
            });
            hlsProcessed.on(Hls.Events.LEVEL_LOADED, (event, data) => {
                log(`処理済み レベル読み込み完了: duration=${data.details?.totalduration?.toFixed(1)}s`, 'info');
            });
            hlsProcessed.on(Hls.Events.ERROR, (event, data) => {
                // Only log fatal errors or non-recoverable issues
                if (data.fatal) {
                    log(`HLS Processed 致命的エラー: ${data.details}`, 'error');
                    // Try to recover
                    if (data.type === Hls.ErrorTypes.MEDIA_ERROR) {
                        hlsProcessed.recoverMediaError();
                    }
                }
                // Silently handle recoverable buffer errors
            });
            hlsProcessed.attachMedia(videoProcessed);
            hlsProcessed.loadSource(processedUrl);
        }

        function loadComparisonDASH(originalUrl, processedUrl, videoOriginal, videoProcessed) {
            // Set muted for autoplay policy
            videoOriginal.muted = true;
            videoProcessed.muted = true;

            // Load original
            dashOriginal = dashjs.MediaPlayer().create();
            dashOriginal.initialize(videoOriginal, originalUrl, true);
            dashOriginal.on(dashjs.MediaPlayer.events.STREAM_INITIALIZED, () => {
                log('オリジナルDASHストリーム読み込み完了', 'success');
            });

            // Load processed
            dashProcessed = dashjs.MediaPlayer().create();
            dashProcessed.initialize(videoProcessed, processedUrl, true);
            dashProcessed.on(dashjs.MediaPlayer.events.STREAM_INITIALIZED, () => {
                log('処理済みDASHストリーム読み込み完了', 'success');
                log('再生開始（ミュート）- 音声を有効にするには動画をクリック', 'info');
            });
        }

        function stopComparisonStreams() {
            if (hlsOriginal) {
                hlsOriginal.destroy();
                hlsOriginal = null;
            }
            if (hlsProcessed) {
                hlsProcessed.destroy();
                hlsProcessed = null;
            }
            if (dashOriginal) {
                dashOriginal.reset();
                dashOriginal = null;
            }
            if (dashProcessed) {
                dashProcessed.reset();
                dashProcessed = null;
            }
            if (syncInterval) {
                clearInterval(syncInterval);
                syncInterval = null;
            }

            const videoOriginal = document.getElementById('videoOriginal');
            const videoProcessed = document.getElementById('videoProcessed');
            videoOriginal.src = '';
            videoProcessed.src = '';
        }

        function startVideoSync(video1, video2) {
            const syncIndicator = document.querySelector('.sync-indicator');
            const syncDiff = document.getElementById('syncDiff');
            let isSyncing = false; // Prevent infinite loop

            // Helper to sync without triggering loops
            const syncTo = (source, target) => {
                if (isSyncing) return;
                isSyncing = true;
                target.currentTime = source.currentTime;
                if (!source.paused && target.paused) {
                    target.play().catch(() => {});
                } else if (source.paused && !target.paused) {
                    target.pause();
                }
                setTimeout(() => { isSyncing = false; }, 100);
            };

            // Only video1 (original) controls sync - video2 follows
            video1.addEventListener('play', () => {
                if (!isSyncing) {
                    isSyncing = true;
                    video2.play().catch(() => {});
                    setTimeout(() => { isSyncing = false; }, 100);
                }
            });
            video1.addEventListener('pause', () => {
                if (!isSyncing) {
                    isSyncing = true;
                    video2.pause();
                    setTimeout(() => { isSyncing = false; }, 100);
                }
            });
            video1.addEventListener('seeked', () => {
                if (!isSyncing) {
                    isSyncing = true;
                    video2.currentTime = video1.currentTime;
                    setTimeout(() => { isSyncing = false; }, 100);
                }
            });

            // Periodic sync check - less aggressive
            syncInterval = setInterval(() => {
                if (isSyncing) return;

                const diff = Math.abs(video1.currentTime - video2.currentTime);
                syncDiff.textContent = `差分: ${diff.toFixed(2)}s`;

                if (diff > 1.0) {
                    // Re-sync only if drift is very large
                    isSyncing = true;
                    video2.currentTime = video1.currentTime;
                    syncIndicator.className = 'sync-indicator warning';
                    setTimeout(() => { isSyncing = false; }, 200);
                } else if (diff > 0.3) {
                    syncIndicator.className = 'sync-indicator warning';
                } else {
                    syncIndicator.className = 'sync-indicator';
                }
            }, 1000); // Check less frequently
        }

        // Test video configurations
        const testVideos = {
            'test30s': {
                name: 'テスト動画 (30秒)',
                original: '/output_original/hls/master.m3u8',
                processed: '/output_processed/hls/master.m3u8'
            },
            'bbb': {
                name: 'Big Buck Bunny (旧フィルター)',
                original: '/output_bbb_original/hls/master.m3u8',
                processed: '/output_bbb/hls/master.m3u8'
            },
            'bbb_v2': {
                name: 'Big Buck Bunny (v2 - 業界標準)',
                original: '/output_bbb_original/hls/master.m3u8',
                processed: '/output_bbb_v2/hls/master.m3u8'
            },
            'bbb_v3': {
                name: 'Big Buck Bunny (v3 - 最小介入)',
                original: '/output_bbb_original/hls/master.m3u8',
                processed: '/output_bbb_v3/hls/master.m3u8'
            }
        };

        function selectTestVideo() {
            const select = document.getElementById('testVideoSelect');
            const videoKey = select.value;

            if (!videoKey) return;

            const video = testVideos[videoKey];
            const baseUrl = 'http://localhost:8080';

            document.getElementById('originalUrl').value = baseUrl + video.original;
            document.getElementById('processedUrl').value = baseUrl + video.processed;

            log(`テスト動画選択: ${video.name}`, 'info');

            // Auto-load if in comparison mode
            if (comparisonMode !== 'normal') {
                loadComparisonStreams();
            }
        }

        // Select source video from outputs directory
        function selectSourceVideo() {
            const select = document.getElementById('sourceVideoSelect');
            const sourceName = select.value;

            if (!sourceName) return;

            const baseUrl = 'http://localhost:8080';
            const streamType = currentStreamType;

            let url;
            if (streamType === 'hls') {
                url = `${baseUrl}/outputs/${sourceName}/hls/master.m3u8`;
            } else {
                url = `${baseUrl}/outputs/${sourceName}/dash/manifest.mpd`;
            }

            document.getElementById('manifestUrl').value = url;
            log(`ソース動画選択: ${sourceName}`, 'info');

            // Auto-load the stream
            loadStream();
        }

        function quickLoadComparison() {
            const baseUrl = 'http://localhost:8080';
            const streamType = currentStreamType;

            if (streamType === 'hls') {
                document.getElementById('originalUrl').value = `${baseUrl}/output_original/hls/master.m3u8`;
                document.getElementById('processedUrl').value = `${baseUrl}/output_processed/hls/master.m3u8`;
            } else {
                document.getElementById('originalUrl').value = `${baseUrl}/output_original/dash/manifest.mpd`;
                document.getElementById('processedUrl').value = `${baseUrl}/output_processed/dash/manifest.mpd`;
            }

            // Ensure comparison mode is active before loading
            if (comparisonMode === 'normal') {
                setComparisonMode('sidebyside');
            }

            loadComparisonStreams();
        }

        // ========================================
        // Filter Toggle Functions
        // ========================================

        // Filter UI is now read-only (info display only)
        // Filters are applied during encoding, not playback

        // ========================================
        // Dynamic Video List Functions
        // ========================================

        async function fetchVideoList() {
            try {
                const response = await fetch('/api/videos');
                if (!response.ok) {
                    throw new Error(`HTTP ${response.status}`);
                }
                const data = await response.json();
                updateVideoSelect(data.videos);
                log(`動画一覧を取得: ${data.videos.length}件`, 'info');
            } catch (error) {
                log(`動画一覧の取得に失敗: ${error.message}`, 'warning');
                // Keep the static options if API fails
            }
        }

        function updateVideoSelect(videos) {
            const select = document.getElementById('sourceVideoSelect');
            if (!select) return;

            // Keep the first "select" option
            const defaultOption = select.options[0];
            select.innerHTML = '';
            select.appendChild(defaultOption);

            // Sort videos: ready first, then encoding, then pending
            const statusOrder = { 'ready': 0, 'encoding': 1, 'pending': 2 };
            videos.sort((a, b) => {
                const orderA = statusOrder[a.status] ?? 3;
                const orderB = statusOrder[b.status] ?? 3;
                if (orderA !== orderB) return orderA - orderB;
                return a.name.localeCompare(b.name);
            });

            videos.forEach(video => {
                const option = document.createElement('option');
                option.value = video.name;

                // Status indicator
                let statusIcon = '';
                let statusText = '';
                if (video.status === 'ready') {
                    statusIcon = '✅';
                    statusText = '';
                } else if (video.status === 'encoding') {
                    statusIcon = '🔄';
                    statusText = ' (エンコード中)';
                } else {
                    statusIcon = '⏳';
                    statusText = ' (未処理)';
                }

                option.textContent = `${statusIcon} ${video.name}${statusText}`;
                option.disabled = video.status !== 'ready';
                select.appendChild(option);
            });
        }

        // Auto-refresh video list every 10 seconds
        let videoListRefreshInterval = null;

        function startVideoListRefresh() {
            if (videoListRefreshInterval) return;
            videoListRefreshInterval = setInterval(fetchVideoList, 10000);
        }

        function stopVideoListRefresh() {
            if (videoListRefreshInterval) {
                clearInterval(videoListRefreshInterval);
                videoListRefreshInterval = null;
            }
        }

        // Initialize
        initBitrateGraph();
        fetchVideoList();
        startVideoListRefresh();
        log('Player ready', 'success');
        log('Keyboard: Space=Play/Pause, F=Fullscreen, M=Mute, 1-4=Select Tier, A/B=Compare', 'info');
