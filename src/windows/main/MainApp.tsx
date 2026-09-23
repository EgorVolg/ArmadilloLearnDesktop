import { useEffect, useRef, useState } from "react";
import "./MainApp.css";
import { Header } from "./components/header/Header";
import { Navbar } from "./components/navbar/Navbar";
import { MainContent } from "./components/main-content/MainContent";
import {
  deleteWord,
  errorText,
  listWords,
  onWordsChanged,
  SavedWord,
  UpdateWord,
  updateWord,
} from "../../shared";

export const MainApp = () => {
  const [selected] = useState(false);

  const [openedLink, setOpenedLink] = useState(1);

  // Текст из строки поиска.
  const [value, setValue] = useState("");

  // Слова из базы, id выбранного слова и ошибка загрузки.
  const [words, setWords] = useState<SavedWord[]>([]);
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const [loadError, setLoadError] = useState("");

  // Увеличиваем это число, когда нужно перечитать список из базы.
  const [reloadKey, setReloadKey] = useState(0);

  // Номер последнего запроса. Ответ на старый запрос
  // (например, на «ar», когда уже набрано «arm») игнорируем.
  const requestIdRef = useRef(0);

  // Загружаем список при старте, при изменении поиска и после reload.
  useEffect(() => {
    const requestId = ++requestIdRef.current;

    listWords(value)
      .then((result) => {
        if (requestId !== requestIdRef.current) return;

        setWords(result);
        setLoadError("");
      })
      .catch((e) => {
        if (requestId !== requestIdRef.current) return;

        setLoadError(errorText(e));
      });
  }, [value, reloadKey]);

  // Слово сохранили/изменили/удалили (в том числе в оверлее) —
  // перечитываем список.
  useEffect(() => {
    const unlisten = onWordsChanged(() => {
      setReloadKey((key) => key + 1);
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  // Выбранное слово. Если его нет в списке (удалили или отфильтровал
  // поиск) — показываем первое.
  const selectedWord =
    words.find((word) => word.id === selectedId) ?? words[0] ?? null;

  function openLink(id: number) {
    setOpenedLink(id);
  }

  // Ошибки из этих функций ловит MainContent и показывает пользователю.
  async function handleUpdate(id: number, changes: UpdateWord) {
    await updateWord(id, changes);
  }

  async function handleDelete(id: number) {
    await deleteWord(id);
    setSelectedId(null);
  }

  return (
    <div className="main-app">
      <Header openedLink={openedLink} onOpenLink={openLink} />
      <Navbar value={value} onChange={setValue} />
      <MainContent
        selected={selected}
        words={words}
        selectedWord={selectedWord}
        search={value}
        loadError={loadError}
        onSelect={setSelectedId}
        onUpdate={handleUpdate}
        onDelete={handleDelete}
      />
    </div>
  );
};
