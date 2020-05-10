/*
Copyright 2017 Takashi Ogura

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
*/

use std::io;
use thiserror::Error;

#[derive(Debug)]
pub enum CollisionPart {
    Start,
    End,
}

#[derive(Debug, Error)]
/// Error for `gear`
pub enum Error {
    #[error("{}", error)]
    Other { error: String },
    #[error("Node name {} not found", .0)]
    NotFound(String),
    #[error(
        "Collision error: {:?} is colliding ({:?})",
        collision_link_names,
        part
    )]
    Collision {
        part: CollisionPart,
        collision_link_names: Vec<String>,
    },
    #[error("IO error {:?}", source)]
    Io {
        #[from]
        source: io::Error,
    },
    #[error("DoF mismatch {} != {}", .0, .1)]
    DofMismatch(usize, usize),
    #[error("URDF error: {:?}", source)]
    Urdf {
        #[from]
        source: urdf_rs::UrdfError,
    },
    #[error("Path not found {}", .0)]
    PathPlanFail(String),
    #[error("Kinematics error: {:?}", source)]
    KinematicsError {
        #[from]
        source: k::Error,
    },
    #[error("failed to parse {}", .0)]
    ParseError(String),
    #[error("Mesh error {}", .0)]
    MeshError(String),
}

/// Result for `gear`
pub type Result<T> = ::std::result::Result<T, Error>;
