"use client";

import { invoke } from "@tauri-apps/api/core";
import { useState, useEffect } from "react";
import Link from "next/link";

interface MTGAMatch {
  id: number;
  controller_player_name: string;
  opponent_player_name: string;
  created_at: string;
}

function formatDate(dateString: string): string {
  const date = new Date(dateString);
  return date.toLocaleString('en-US', {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit'
  });
}

export default function MatchesTable() {
  const [matches, setMatches] = useState<MTGAMatch[]>([]);

  useEffect(() => {
    invoke<MTGAMatch[]>("command_matches", {})
      .then((result) => {
        setMatches(result);
      })
      .catch(console.error);
  }, []);

  return (
    <table>
      <thead>
        <tr>
          <th>Controller</th>
          <th>Opponent</th>
          <th>Created At</th>
        </tr>
      </thead>
      <tbody>
        {matches.map((match, index) => (
          <tr
            className="text-center hover:bg-[rgb(var(--hover-color))] transition-colors duration-200"
            key={index}
            onClick={() =>
              (window.location.href = `/match-details?id=${match.id}`)
            }
            style={{ cursor: "pointer" }}
          >
            <td>{match.controller_player_name}</td>
            <td>{match.opponent_player_name}</td>
            <td>{formatDate(match.created_at)}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}
