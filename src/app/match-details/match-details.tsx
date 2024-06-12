'use client'

import { invoke} from "@tauri-apps/api/tauri";
import {useEffect, useState} from "react";

interface DeckList {
    game_number: number;
    deck: string[];
    sideboard: string[];
}

interface Mulligan {
    hand: string[];
    opponent_identity: string;
    game_number: number;
    number_to_keep: number;
    play_draw: string;
    decision: string;
}

interface MatchDetails {
    id: number;
    did_controller_win: boolean;
    controller_player_name: string;
    opponent_player_name: string;
    decklists: DeckList[];
    mulligans: Mulligan[];
}

export default function MatchDetails() {
    const [match, setMatch] = useState<MatchDetails | null>(null);
    useEffect(() => {
        let params = new URLSearchParams(document.location.search);
        let id = params.get("id");
        if (id !== null) {
            invoke<MatchDetails>('get_match_details', {matchId: id})
                .then(result => {
                    console.log(result)
                    setMatch(result);
                })
                .catch(console.error)
        } else {
            setMatch({
                id: 0,
                did_controller_win: false,
                controller_player_name: "",
                opponent_player_name: "",
                decklists: [],
                mulligans: []
            });
        }
    }, [])

    return (
        <div>
            <h1>Match Details</h1>
            {match && (
                <div>
                    <div>
                        <p>Match ID: {match.id}</p>
                        <p>Controller: {match.controller_player_name}</p>
                        <p>Opponent: {match.opponent_player_name}</p>
                        <p>Winner: {match.did_controller_win ? match.controller_player_name : match.opponent_player_name}</p>
                    </div>
                    <div>
                        <h2>Decklists</h2>
                        {match.decklists.map((decklist, index) => (
                            <div key={index}>
                                <h3>Game {decklist.game_number}</h3>
                                <p>Deck: {decklist.deck.join(", ")}</p>
                                <p>Sideboard: {decklist.sideboard.join(", ")}</p>
                            </div>
                        ))}
                    </div>
                    <div>
                        <h2>Mulligans</h2>
                        {match.mulligans.map((mulligan, index) => (
                            <div key={index}>
                                <h3>Game {mulligan.game_number}</h3>
                                <p>Hand: {mulligan.hand.join(", ")}</p>
                                <p>Opponent Identity: {mulligan.opponent_identity}</p>
                                <p>Number to Keep: {mulligan.number_to_keep}</p>
                                <p>Play/Draw: {mulligan.play_draw}</p>
                                <p>Decision: {mulligan.decision}</p>
                            </div>
                        ))}
                    </div>
                </div>
            )}
        </div>
    )
}