import init, {
  wasm_init_engine,
  wasm_get_canvas_width,
  wasm_get_canvas_height,
  wasm_get_framebuffer_ptr,
  wasm_on_key_down,
  wasm_on_key_up,
  wasm_on_mouse_move,
  wasm_on_mouse_down,
  wasm_on_mouse_up,
  wasm_render_frame,
  wasm_get_score,
  wasm_get_fps,
  wasm_get_cpu_us,
  wasm_get_spatial_queries,
  wasm_toggle_radar,
  wasm_toggle_spatial_grid,
  wasm_toggle_editor_mode,
  wasm_get_status_message,
  wasm_export_standalone_html,
  wasm_export_game_package,
  wasm_import_game_package,
  wasm_validate_recipe,
  wasm_apply_recipe
} from './pkg/karactirage_wasm.js';

// Table de correspondance des touches du navigateur vers les codes d'entrée WebKey
const KEY_MAP = {
  'ArrowLeft': 0,
  'ArrowRight': 1,
  'ArrowUp': 2,
  'ArrowDown': 3,
  'KeyW': 4,
  'KeyA': 5,
  'KeyS': 6,
  'KeyD': 7,
  'KeyZ': 8,
  'KeyQ': 9,
  'KeyE': 10,
  'Space': 11,
  'Tab': 12,
  'KeyN': 13,
  'KeyG': 14,
  'KeyM': 15,
  'F1': 16,
  'F2': 17,
  'F3': 18,
  'KeyR': 19,
  'Escape': 20,
  'ControlLeft': 21,
  'ControlRight': 21,
  'MetaLeft': 21,
  'MetaRight': 21,
  'KeyV': 22,
  'KeyB': 23,
  'KeyX': 24,
  'KeyT': 25,
  'KeyP': 26,
  'KeyH': 27,
  'KeyU': 28,
  'F4': 29,
  'KeyL': 29,
  'Digit1': 30,
  'Digit2': 31,
  'Digit3': 32,
  'Digit4': 33,
  'Digit5': 34
};

async function main() {
  const loadingOverlay = document.getElementById('loading-overlay');
  const loadingText = document.getElementById('loading-text');
  const canvas = document.getElementById('game-canvas');
  const ctx = canvas.getContext('2d', { alpha: false });

  // Éléments du HUD
  const statusEl = document.getElementById('engine-status');
  const fpsEl = document.getElementById('metric-fps');
  const cpuEl = document.getElementById('metric-cpu');
  const budgetEl = document.getElementById('metric-budget');
  const spatialEl = document.getElementById('metric-spatial');
  const scoreEl = document.getElementById('metric-score');
  const modeEl = document.getElementById('metric-mode');

  try {
    loadingText.textContent = "Téléchargement & initialisation de karactirage_wasm_bg.wasm...";
    const wasm = await init();
    
    loadingText.textContent = "Démarrage du moteur et chargement des scripts KVL...";
    const initRes = wasm_init_engine();
    if (initRes !== 0) {
      throw new Error(`Échec de wasm_init_engine: code ${initRes}`);
    }

    const width = wasm_get_canvas_width();
    const height = wasm_get_canvas_height();
    canvas.width = width;
    canvas.height = height;

    loadingOverlay.classList.add('hidden');
    statusEl.textContent = wasm_get_status_message() || "Prêt ! ZQSD/Flèches pour jouer.";

    // ==========================================
    // GESTION DES ENTRÉES CLAVIER & SOURIS
    // ==========================================
    window.addEventListener('keydown', (e) => {
      if (KEY_MAP.hasOwnProperty(e.code)) {
        if (['Space', 'Tab', 'ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight', 'F1', 'F2', 'F3'].includes(e.code)) {
          e.preventDefault();
        }
        wasm_on_key_down(KEY_MAP[e.code]);
      }
    });

    window.addEventListener('keyup', (e) => {
      if (KEY_MAP.hasOwnProperty(e.code)) {
        wasm_on_key_up(KEY_MAP[e.code]);
      }
    });

    const getCanvasCoordinates = (e) => {
      const rect = canvas.getBoundingClientRect();
      const scaleX = canvas.width / rect.width;
      const scaleY = canvas.height / rect.height;
      return {
        x: (e.clientX - rect.left) * scaleX,
        y: (e.clientY - rect.top) * scaleY,
      };
    };

    canvas.addEventListener('mousemove', (e) => {
      const coords = getCanvasCoordinates(e);
      wasm_on_mouse_move(coords.x, coords.y);
    });

    canvas.addEventListener('mousedown', (e) => {
      const coords = getCanvasCoordinates(e);
      wasm_on_mouse_move(coords.x, coords.y);
      wasm_on_mouse_down(e.button);
    });

    canvas.addEventListener('mouseup', (e) => {
      wasm_on_mouse_up(e.button);
    });

    canvas.addEventListener('contextmenu', (e) => {
      e.preventDefault();
    });

    const triggerKey = (code) => {
      if (KEY_MAP.hasOwnProperty(code)) {
        wasm_on_key_down(KEY_MAP[code]);
        setTimeout(() => wasm_on_key_up(KEY_MAP[code]), 60);
      }
    };

    // Boutons rapides du HUD et de la barre d'outils
    const bindBtn = (id, fn) => {
      const el = document.getElementById(id);
      if (el) el.addEventListener('click', fn);
    };

    bindBtn('btn-toggle-editor', () => {
      const isEdit = wasm_toggle_editor_mode();
      modeEl.textContent = isEdit ? "ÉDITION" : "JEU";
      modeEl.className = isEdit ? "metric-value text-amber" : "metric-value text-purple";
    });

    bindBtn('btn-toggle-radar', () => wasm_toggle_radar());
    bindBtn('btn-toggle-grid', () => wasm_toggle_spatial_grid());
    bindBtn('btn-toggle-audio', () => triggerKey('KeyM'));
    bindBtn('btn-reload', () => triggerKey('KeyR'));
    bindBtn('btn-tool-select', () => triggerKey('KeyV'));
    bindBtn('btn-tool-brush', () => triggerKey('KeyB'));
    bindBtn('btn-tool-eraser', () => triggerKey('KeyX'));
    bindBtn('btn-tool-tile', () => triggerKey('KeyT'));
    bindBtn('btn-tool-spawn', () => triggerKey('KeyP'));
    bindBtn('btn-tool-help', () => triggerKey('KeyH'));
    bindBtn('btn-tool-genre', () => triggerKey('F4'));
    bindBtn('btn-tool-use', () => triggerKey('KeyU'));
    bindBtn('btn-slot-1', () => triggerKey('Digit1'));
    bindBtn('btn-slot-2', () => triggerKey('Digit2'));
    bindBtn('btn-slot-3', () => triggerKey('Digit3'));
    bindBtn('btn-slot-4', () => triggerKey('Digit4'));
    bindBtn('btn-slot-5', () => triggerKey('Digit5'));

    // ==========================================
    // EXPORTATEUR STANDALONE & IMPORTATION WEB
    // ==========================================
    function downloadBlob(content, filename, contentType) {
      const blob = new Blob([content], { type: contentType });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = filename;
      document.body.appendChild(a);
      a.click();
      document.body.removeChild(a);
      URL.revokeObjectURL(url);
    }

    const exportModal = document.getElementById('export-modal');
    const btnExportGame = document.getElementById('btn-export-game');
    const btnCloseExportModal = document.getElementById('btn-close-export-modal');
    const btnConfirmExport = document.getElementById('btn-confirm-export');
    const exportCards = document.querySelectorAll('.export-option-card');
    const inputTitle = document.getElementById('export-game-title');
    const inputAuthor = document.getElementById('export-game-author');
    const btnTriggerImport = document.getElementById('btn-trigger-import');
    const inputImportPackage = document.getElementById('input-import-package');

    let selectedFormat = 'html';

    exportCards.forEach(card => {
      card.addEventListener('click', () => {
        exportCards.forEach(c => c.classList.remove('active'));
        card.classList.add('active');
        selectedFormat = card.getAttribute('data-format');
      });
    });

    if (btnExportGame) {
      btnExportGame.addEventListener('click', () => {
        if (exportModal) exportModal.classList.remove('hidden');
      });
    }

    if (btnCloseExportModal) {
      btnCloseExportModal.addEventListener('click', () => {
        if (exportModal) exportModal.classList.add('hidden');
      });
    }

    if (btnConfirmExport) {
      btnConfirmExport.addEventListener('click', () => {
        const title = (inputTitle && inputTitle.value.trim()) || "Mon Jeu KaractiRage";
        const author = (inputAuthor && inputAuthor.value.trim()) || "Atelier Créateur";
        const safeName = title.toLowerCase().replace(/[^a-z0-9]/gi, '_');

        if (selectedFormat === 'html') {
          const htmlContent = wasm_export_standalone_html(title, author);
          downloadBlob(htmlContent, `${safeName}_standalone.html`, 'text/html;charset=utf-8');
          statusEl.textContent = `Jeu autonome exporté avec succès : ${safeName}_standalone.html !`;
        } else if (selectedFormat === 'json' || selectedFormat === 'desktop') {
          const jsonContent = wasm_export_game_package(title, author);
          downloadBlob(jsonContent, `${safeName}.karacti.json`, 'application/json;charset=utf-8');
          statusEl.textContent = `Package universel exporté : ${safeName}.karacti.json !`;
        }
        if (exportModal) exportModal.classList.add('hidden');
      });
    }

    if (btnTriggerImport && inputImportPackage) {
      btnTriggerImport.addEventListener('click', () => {
        inputImportPackage.click();
      });

      inputImportPackage.addEventListener('change', (e) => {
        const file = e.target.files && e.target.files[0];
        if (!file) return;

        const reader = new FileReader();
        reader.onload = (event) => {
          try {
            const content = event.target.result;
            const res = wasm_import_game_package(content);
            if (res === 0) {
              statusEl.textContent = `Package "${file.name}" importé et appliqué avec succès !`;
              if (exportModal) exportModal.classList.add('hidden');
            } else {
              alert(`Erreur lors de l'importation du package (Code: ${res})`);
            }
          } catch (err) {
            alert(`Erreur de lecture du package : ${err.message}`);
          }
        };
        reader.readAsText(file);
      });
    }

    // ==========================================
    // ASSISTANT DE CRÉATION DE JEU (GAME RECIPE)
    // ==========================================
    const recipeModal = document.getElementById('recipe-modal');
    const btnOpenRecipeWizard = document.getElementById('btn-open-recipe-wizard');
    const btnCloseRecipeModal = document.getElementById('btn-close-recipe-modal');
    const btnRecipePrev = document.getElementById('btn-recipe-prev');
    const btnRecipeNext = document.getElementById('btn-recipe-next');
    const btnRecipeApply = document.getElementById('btn-recipe-apply');
    const btnExportRecipeJson = document.getElementById('btn-export-recipe-json');
    const btnImportRecipeJson = document.getElementById('btn-import-recipe-json');
    const inputImportRecipe = document.getElementById('input-import-recipe');
    const stepIndicators = document.querySelectorAll('.step-indicator');
    const stepViews = document.querySelectorAll('.recipe-step-view');
    const inputRecipeTitle = document.getElementById('recipe-game-title');
    const inputRecipeAuthor = document.getElementById('recipe-game-author');
    const selectSecondaryGenre = document.getElementById('recipe-secondary-genre');
    const auditBadge = document.getElementById('recipe-audit-badge');
    const auditDetails = document.getElementById('recipe-audit-details');

    let currentRecipeStep = 1;

    let recipeData = {
      schema_version: 1,
      title: "Mon Aventure Épique",
      author: "Atelier KaractiRage",
      genres: {
        primary: "adventure",
        secondary: null
      },
      dimension: "top_down",
      camera: "follow_player",
      player: {
        name: "Héros",
        speed: 180,
        max_health: 5,
        directions: "four_directions",
        abilities: {
          jump: false,
          dash: false,
          shoot: false,
          melee: true,
          interact: true
        },
        combat: "melee"
      },
      world: {
        gravity_enabled: false,
        day_night_cycle: false,
        dynamic_lighting: false,
        interactive_elements: ["collectible", "door", "chest"]
      },
      gameplay: {
        inventory: true,
        dialogues: true,
        quest_objectives: true,
        save_system: true,
        score_system: true,
        boss_fight: false,
        wave_spawner: false
      },
      performance: "low_end"
    };

    function updateRecipeStepperUI() {
      stepIndicators.forEach(ind => {
        const step = parseInt(ind.getAttribute('data-step'), 10);
        ind.classList.remove('active', 'done');
        if (step === currentRecipeStep) {
          ind.classList.add('active');
        } else if (step < currentRecipeStep) {
          ind.classList.add('done');
        }
      });

      stepViews.forEach((view, idx) => {
        if (idx + 1 === currentRecipeStep) {
          view.classList.add('active');
        } else {
          view.classList.remove('active');
        }
      });

      if (btnRecipePrev) {
        btnRecipePrev.disabled = currentRecipeStep === 1;
      }

      if (currentRecipeStep === 6) {
        if (btnRecipeNext) btnRecipeNext.classList.add('hidden');
        if (btnRecipeApply) btnRecipeApply.classList.remove('hidden');
        runRecipeAudit();
      } else {
        if (btnRecipeNext) btnRecipeNext.classList.remove('hidden');
        if (btnRecipeApply) btnRecipeApply.classList.add('hidden');
      }
    }

    function runRecipeAudit() {
      if (inputRecipeTitle) recipeData.title = inputRecipeTitle.value.trim() || "Mon Jeu";
      if (inputRecipeAuthor) recipeData.author = inputRecipeAuthor.value.trim() || "Atelier Créateur";

      try {
        const jsonStr = JSON.stringify(recipeData);
        const reportJson = wasm_validate_recipe(jsonStr);
        const report = JSON.parse(reportJson);

        if (auditBadge) {
          if (report.is_valid) {
            auditBadge.className = "badge-audit-ok";
            auditBadge.textContent = "✓ RECETTE CONFORME & PRÊTE";
          } else {
            auditBadge.className = "badge-audit-warn";
            auditBadge.textContent = "⚠️ CONFLIT DÉTECTÉ";
          }
        }

        if (auditDetails) {
          let html = '';
          if (report.resolved_dependencies && report.resolved_dependencies.length > 0) {
            report.resolved_dependencies.forEach(dep => {
              html += `<div class="audit-item text-emerald">● ${dep}</div>`;
            });
          }
          if (report.warnings && report.warnings.length > 0) {
            report.warnings.forEach(w => {
              html += `<div class="audit-item text-amber">▲ Attention : ${w}</div>`;
            });
          }
          if (report.conflicts && report.conflicts.length > 0) {
            report.conflicts.forEach(c => {
              html += `<div class="audit-item text-rose">✕ Conflit : ${c}</div>`;
            });
          }
          if (!html) {
            html = '<div class="audit-item text-emerald">● Tous les paramètres de la recette sont équilibrés.</div>';
          }
          auditDetails.innerHTML = html;
        }
      } catch (err) {
        console.error("Erreur d'audit de la recette :", err);
      }
    }

    // Gestion du clic sur les étapes du stepper
    stepIndicators.forEach(ind => {
      ind.addEventListener('click', () => {
        const step = parseInt(ind.getAttribute('data-step'), 10);
        currentRecipeStep = step;
        updateRecipeStepperUI();
      });
    });

    if (btnRecipePrev) {
      btnRecipePrev.addEventListener('click', () => {
        if (currentRecipeStep > 1) {
          currentRecipeStep--;
          updateRecipeStepperUI();
        }
      });
    }

    if (btnRecipeNext) {
      btnRecipeNext.addEventListener('click', () => {
        if (currentRecipeStep < 6) {
          currentRecipeStep++;
          updateRecipeStepperUI();
        }
      });
    }

    if (btnOpenRecipeWizard) {
      btnOpenRecipeWizard.addEventListener('click', () => {
        if (recipeModal) {
          recipeModal.classList.remove('hidden');
          currentRecipeStep = 1;
          updateRecipeStepperUI();
        }
      });
    }

    if (btnCloseRecipeModal) {
      btnCloseRecipeModal.addEventListener('click', () => {
        if (recipeModal) recipeModal.classList.add('hidden');
      });
    }

    // Sélection des genres principaux
    const genreCards = document.querySelectorAll('#primary-genres-grid .genre-card');
    genreCards.forEach(card => {
      card.addEventListener('click', () => {
        genreCards.forEach(c => c.classList.remove('active'));
        card.classList.add('active');
        recipeData.genres.primary = card.getAttribute('data-genre');
      });
    });

    if (selectSecondaryGenre) {
      selectSecondaryGenre.addEventListener('change', (e) => {
        recipeData.genres.secondary = e.target.value === 'none' ? null : e.target.value;
      });
    }

    // Sélection Dimension
    const dimCards = document.querySelectorAll('#dimension-grid .choice-card');
    dimCards.forEach(card => {
      card.addEventListener('click', () => {
        dimCards.forEach(c => c.classList.remove('active'));
        card.classList.add('active');
        recipeData.dimension = card.getAttribute('data-dimension');
      });
    });

    // Sélection Caméra
    const camCards = document.querySelectorAll('#camera-grid .choice-card');
    camCards.forEach(card => {
      card.addEventListener('click', () => {
        camCards.forEach(c => c.classList.remove('active'));
        card.classList.add('active');
        recipeData.camera = card.getAttribute('data-camera');
      });
    });

    // Sélection Déplacement
    const moveCards = document.querySelectorAll('#movement-grid .choice-card');
    moveCards.forEach(card => {
      card.addEventListener('click', () => {
        moveCards.forEach(c => c.classList.remove('active'));
        card.classList.add('active');
        recipeData.player.directions = card.getAttribute('data-move');
      });
    });

    // Checkboxes Capacités
    function syncAbilitiesFromUI() {
      recipeData.player.abilities.jump = !!document.getElementById('chk-ability-jump')?.checked;
      recipeData.player.abilities.dash = !!document.getElementById('chk-ability-dash')?.checked;
      recipeData.player.abilities.shoot = !!document.getElementById('chk-ability-shoot')?.checked;
      recipeData.player.abilities.melee = !!document.getElementById('chk-ability-melee')?.checked;
      recipeData.player.abilities.interact = !!document.getElementById('chk-ability-interact')?.checked;
    }

    ['chk-ability-jump', 'chk-ability-dash', 'chk-ability-shoot', 'chk-ability-melee', 'chk-ability-interact'].forEach(id => {
      const el = document.getElementById(id);
      if (el) el.addEventListener('change', syncAbilitiesFromUI);
    });

    // Monde Toggles
    const chkGrav = document.getElementById('chk-world-gravity');
    if (chkGrav) chkGrav.addEventListener('change', (e) => { recipeData.world.gravity_enabled = e.target.checked; });
    const chkDayNight = document.getElementById('chk-world-daynight');
    if (chkDayNight) chkDayNight.addEventListener('change', (e) => { recipeData.world.day_night_cycle = e.target.checked; });
    const chkLighting = document.getElementById('chk-world-lighting');
    if (chkLighting) chkLighting.addEventListener('change', (e) => { recipeData.world.dynamic_lighting = e.target.checked; });

    // Éléments du monde
    function syncWorldElementsFromUI() {
      const list = [];
      if (document.getElementById('chk-elem-collectible')?.checked) list.push('collectible');
      if (document.getElementById('chk-elem-door')?.checked) list.push('door');
      if (document.getElementById('chk-elem-chest')?.checked) list.push('chest');
      if (document.getElementById('chk-elem-lever')?.checked) list.push('lever');
      if (document.getElementById('chk-elem-trap')?.checked) list.push('trap');
      if (document.getElementById('chk-elem-moving-plat')?.checked) list.push('moving_platform');
      recipeData.world.interactive_elements = list;
    }

    ['chk-elem-collectible', 'chk-elem-door', 'chk-elem-chest', 'chk-elem-lever', 'chk-elem-trap', 'chk-elem-moving-plat'].forEach(id => {
      const el = document.getElementById(id);
      if (el) el.addEventListener('change', syncWorldElementsFromUI);
    });

    // Fonctionnalités Gameplay
    function syncGameplayFeaturesFromUI() {
      recipeData.gameplay.inventory = !!document.getElementById('chk-feat-inventory')?.checked;
      recipeData.gameplay.dialogues = !!document.getElementById('chk-feat-dialogues')?.checked;
      recipeData.gameplay.quest_objectives = !!document.getElementById('chk-feat-quests')?.checked;
      recipeData.gameplay.boss_fight = !!document.getElementById('chk-feat-boss')?.checked;
      recipeData.gameplay.score_system = !!document.getElementById('chk-feat-score')?.checked;
      recipeData.gameplay.wave_spawner = !!document.getElementById('chk-feat-waves')?.checked;
    }

    ['chk-feat-inventory', 'chk-feat-dialogues', 'chk-feat-quests', 'chk-feat-boss', 'chk-feat-score', 'chk-feat-waves'].forEach(id => {
      const el = document.getElementById(id);
      if (el) el.addEventListener('change', syncGameplayFeaturesFromUI);
    });

    // Sélection Performance
    const perfCards = document.querySelectorAll('#perf-grid .choice-card');
    perfCards.forEach(card => {
      card.addEventListener('click', () => {
        perfCards.forEach(c => c.classList.remove('active'));
        card.classList.add('active');
        recipeData.performance = card.getAttribute('data-perf');
        runRecipeAudit();
      });
    });

    // Presets d'Archetypes Rapides (1 Clic)
    const presetArchetypes = {
      rpg_dungeon: {
        title: "Donjon & Énigmes",
        genre: "adventure",
        secGenre: "rpg",
        dimension: "top_down",
        camera: "follow_player",
        directions: "four_directions",
        jump: false, dash: false, shoot: false, melee: true, interact: true,
        gravity: false,
        elements: ["collectible", "door", "chest", "lever"],
        inventory: true, dialogues: true, quests: true, boss: false, waves: false
      },
      platformer: {
        title: "Cyber Platformer 2D",
        genre: "platformer",
        secGenre: "action",
        dimension: "side_view",
        camera: "follow_player",
        directions: "horizontal_only",
        jump: true, dash: true, shoot: false, melee: false, interact: true,
        gravity: true,
        elements: ["collectible", "door", "moving_platform", "trap"],
        inventory: false, dialogues: false, quests: true, boss: false, waves: false
      },
      shootemup: {
        title: "Shmup Stellaire",
        genre: "shoot_em_up",
        secGenre: "action",
        dimension: "top_down",
        camera: "follow_player",
        directions: "free_inertia",
        jump: false, dash: true, shoot: true, melee: false, interact: false,
        gravity: false,
        elements: ["collectible"],
        inventory: false, dialogues: false, quests: true, boss: false, waves: true
      },
      puzzle_sokoban: {
        title: "Puzzle Sokoban",
        genre: "puzzle",
        secGenre: "none",
        dimension: "top_down",
        camera: "room_by_room",
        directions: "four_directions",
        jump: false, dash: false, shoot: false, melee: false, interact: true,
        gravity: false,
        elements: ["door", "lever", "collectible"],
        inventory: false, dialogues: false, quests: true, boss: false, waves: false
      },
      boss_arena: {
        title: "Arène du Titan",
        genre: "combat",
        secGenre: "action",
        dimension: "top_down",
        camera: "follow_player",
        directions: "eight_directions",
        jump: false, dash: true, shoot: false, melee: true, interact: false,
        gravity: false,
        elements: ["door"],
        inventory: true, dialogues: false, quests: true, boss: true, waves: false
      }
    };

    function applyArchetypePreset(presetKey) {
      const p = presetArchetypes[presetKey];
      if (!p) return;

      if (inputRecipeTitle) inputRecipeTitle.value = p.title;
      recipeData.title = p.title;
      recipeData.genres.primary = p.genre;
      recipeData.genres.secondary = p.secGenre === 'none' ? null : p.secGenre;
      recipeData.dimension = p.dimension;
      recipeData.camera = p.camera;
      recipeData.player.directions = p.directions;
      recipeData.player.abilities.jump = p.jump;
      recipeData.player.abilities.dash = p.dash;
      recipeData.player.abilities.shoot = p.shoot;
      recipeData.player.abilities.melee = p.melee;
      recipeData.player.abilities.interact = p.interact;
      recipeData.world.gravity_enabled = p.gravity;
      recipeData.world.interactive_elements = p.elements;
      recipeData.gameplay.inventory = p.inventory;
      recipeData.gameplay.dialogues = p.dialogues;
      recipeData.gameplay.quest_objectives = p.quests;
      recipeData.gameplay.boss_fight = p.boss;
      recipeData.gameplay.wave_spawner = p.waves;

      // Synchronisation visuelle de l'UI
      genreCards.forEach(c => c.classList.toggle('active', c.getAttribute('data-genre') === p.genre));
      if (selectSecondaryGenre) selectSecondaryGenre.value = p.secGenre;
      dimCards.forEach(c => c.classList.toggle('active', c.getAttribute('data-dimension') === p.dimension));
      camCards.forEach(c => c.classList.toggle('active', c.getAttribute('data-camera') === p.camera));
      moveCards.forEach(c => c.classList.toggle('active', c.getAttribute('data-move') === p.directions));

      if (document.getElementById('chk-ability-jump')) document.getElementById('chk-ability-jump').checked = p.jump;
      if (document.getElementById('chk-ability-dash')) document.getElementById('chk-ability-dash').checked = p.dash;
      if (document.getElementById('chk-ability-shoot')) document.getElementById('chk-ability-shoot').checked = p.shoot;
      if (document.getElementById('chk-ability-melee')) document.getElementById('chk-ability-melee').checked = p.melee;
      if (document.getElementById('chk-ability-interact')) document.getElementById('chk-ability-interact').checked = p.interact;
      if (chkGrav) chkGrav.checked = p.gravity;

      if (document.getElementById('chk-elem-collectible')) document.getElementById('chk-elem-collectible').checked = p.elements.includes('collectible');
      if (document.getElementById('chk-elem-door')) document.getElementById('chk-elem-door').checked = p.elements.includes('door');
      if (document.getElementById('chk-elem-chest')) document.getElementById('chk-elem-chest').checked = p.elements.includes('chest');
      if (document.getElementById('chk-elem-lever')) document.getElementById('chk-elem-lever').checked = p.elements.includes('lever');
      if (document.getElementById('chk-elem-trap')) document.getElementById('chk-elem-trap').checked = p.elements.includes('trap');
      if (document.getElementById('chk-elem-moving-plat')) document.getElementById('chk-elem-moving-plat').checked = p.elements.includes('moving_platform');

      if (document.getElementById('chk-feat-inventory')) document.getElementById('chk-feat-inventory').checked = p.inventory;
      if (document.getElementById('chk-feat-dialogues')) document.getElementById('chk-feat-dialogues').checked = p.dialogues;
      if (document.getElementById('chk-feat-quests')) document.getElementById('chk-feat-quests').checked = p.quests;
      if (document.getElementById('chk-feat-boss')) document.getElementById('chk-feat-boss').checked = p.boss;
      if (document.getElementById('chk-feat-waves')) document.getElementById('chk-feat-waves').checked = p.waves;

      // Passer directement à l'étape 6 pour valider l'audit
      currentRecipeStep = 6;
      updateRecipeStepperUI();
    }

    document.querySelectorAll('.btn-preset-archetype').forEach(btn => {
      btn.addEventListener('click', () => {
        const presetKey = btn.getAttribute('data-preset');
        applyArchetypePreset(presetKey);
      });
    });

    // Bouton Générer & Jouer le Jeu
    if (btnRecipeApply) {
      btnRecipeApply.addEventListener('click', () => {
        if (inputRecipeTitle) recipeData.title = inputRecipeTitle.value.trim() || "Mon Jeu";
        if (inputRecipeAuthor) recipeData.author = inputRecipeAuthor.value.trim() || "Atelier Créateur";

        try {
          const jsonStr = JSON.stringify(recipeData);
          const res = wasm_apply_recipe(jsonStr);
          if (res === 0) {
            if (recipeModal) recipeModal.classList.add('hidden');
            statusEl.textContent = `Recette "${recipeData.title}" instanciée et active ! Bon jeu.`;
            const projectChip = document.querySelector('.chip-project .chip-text');
            if (projectChip) projectChip.textContent = recipeData.title;
          } else {
            alert(`Impossible d'appliquer la recette (Code d'erreur WASM : ${res})`);
          }
        } catch (err) {
          alert(`Erreur lors de l'application de la recette : ${err.message}`);
        }
      });
    }

    // Bouton Exporter Recette JSON
    if (btnExportRecipeJson) {
      btnExportRecipeJson.addEventListener('click', () => {
        if (inputRecipeTitle) recipeData.title = inputRecipeTitle.value.trim() || "Mon Jeu";
        if (inputRecipeAuthor) recipeData.author = inputRecipeAuthor.value.trim() || "Atelier Créateur";

        const safeName = recipeData.title.toLowerCase().replace(/[^a-z0-9]/gi, '_');
        const jsonContent = JSON.stringify(recipeData, null, 2);
        downloadBlob(jsonContent, `${safeName}.recipe.json`, 'application/json;charset=utf-8');
        statusEl.textContent = `Recette exportée : ${safeName}.recipe.json !`;
      });
    }

    // Bouton Importer Recette JSON
    if (btnImportRecipeJson && inputImportRecipe) {
      btnImportRecipeJson.addEventListener('click', () => {
        inputImportRecipe.click();
      });

      inputImportRecipe.addEventListener('change', (e) => {
        const file = e.target.files && e.target.files[0];
        if (!file) return;

        const reader = new FileReader();
        reader.onload = (event) => {
          try {
            const parsed = JSON.parse(event.target.result);
            if (!parsed.schema_version || !parsed.genres) {
              throw new Error("Format de recette invalide (schéma manquant)");
            }
            recipeData = parsed;
            if (inputRecipeTitle) inputRecipeTitle.value = recipeData.title || "";
            if (inputRecipeAuthor) inputRecipeAuthor.value = recipeData.author || "";
            currentRecipeStep = 6;
            updateRecipeStepperUI();
            statusEl.textContent = `Recette "${recipeData.title}" chargée avec succès !`;
          } catch (err) {
            alert(`Erreur de lecture de la recette : ${err.message}`);
          }
        };
        reader.readAsText(file);
      });
    }


    // ==========================================
    // BOUCLE DE RENDU 60 FPS (requestAnimationFrame)
    // ==========================================
    let lastTime = performance.now();
    let frameCounter = 0;
    let lastMetricUpdate = performance.now();

    function renderLoop(now) {
      const dt = Math.min((now - lastTime) / 1000.0, 0.1);
      lastTime = now;

      // 1. Mise à jour de la physique et rendu dans le buffer RGBA
      wasm_render_frame(dt);

      // 2. Copie directe sans allocation vers le Canvas HTML5
      const ptr = wasm_get_framebuffer_ptr();
      if (ptr !== 0) {
        const pixels = new Uint8ClampedArray(wasm.memory.buffer, ptr, width * height * 4);
        const imageData = new ImageData(pixels, width, height);
        ctx.putImageData(imageData, 0, 0);
      }

      // 3. Rafraîchissement des métriques HUD à 10 Hz
      frameCounter++;
      if (now - lastMetricUpdate >= 100) {
        lastMetricUpdate = now;

        const fps = wasm_get_fps();
        const cpuUs = wasm_get_cpu_us();
        const spatialReq = wasm_get_spatial_queries();
        const score = wasm_get_score();
        const statusMsg = wasm_get_status_message();

        fpsEl.textContent = `${fps.toFixed(1)} FPS`;
        cpuEl.textContent = `${cpuUs} µs`;
        
        if (cpuUs < 16666) {
          budgetEl.textContent = "RESPECTÉ (<16.6ms)";
          budgetEl.className = "metric-value text-emerald";
        } else {
          budgetEl.textContent = "DÉPASSÉ !";
          budgetEl.className = "metric-value text-rose";
        }

        spatialEl.textContent = `${spatialReq} req/frame`;
        scoreEl.textContent = `${score} PTS`;
        if (statusMsg) {
          statusEl.textContent = statusMsg;
        }
      }

      requestAnimationFrame(renderLoop);
    }

    requestAnimationFrame(renderLoop);

  } catch (err) {
    console.error("Erreur d'initialisation WebAssembly:", err);
    loadingText.textContent = `Erreur: ${err.message}`;
    loadingText.style.color = "#ef4444";
  }
}

main();
