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
use k;
use ncollide::shape::Compound3;
use na;
use rrt;
use std::path::Path;
use urdf_rs;

use collision_checker::*;
use errors::*;
use funcs::*;

/// Collision Avoidance Path Planner
pub struct JointPathPlanner<R>
where
    R: k::LinkContainer<f64>,
{
    /// Instance of `k::LinkContainer` to check the collision
    pub collision_check_robot: R,
    /// Collision checker
    pub collision_checker: CollisionChecker<f64>,
    /// Unit length for searching
    ///
    /// If the value is large, the path become sparse.
    pub step_length: f64,
    /// Max num of RRT search loop
    pub max_try: usize,
    /// Num of path smoothing trials
    pub num_smoothing: usize,
    /// The robot instance which is used to create the robot model
    pub urdf_robot: Option<urdf_rs::Robot>,
}

impl<R> JointPathPlanner<R>
where
    R: k::LinkContainer<f64>,
{
    /// Create `JointPathPlanner`
    pub fn new(
        collision_check_robot: R,
        collision_checker: CollisionChecker<f64>,
        step_length: f64,
        max_try: usize,
        num_smoothing: usize,
    ) -> Self {
        JointPathPlanner {
            collision_check_robot,
            collision_checker,
            step_length,
            max_try,
            num_smoothing,
            urdf_robot: None,
        }
    }
    /// Check if the joint_angles are OK
    pub fn is_feasible<K>(
        &mut self,
        using_joints: &mut K,
        joint_angles: &[f64],
        objects: &Compound3<f64>,
    ) -> bool
    where
        K: k::JointContainer<f64>,
    {
        if using_joints.set_joint_angles(joint_angles).is_err() {
            return false;
        }
        !self.has_any_colliding(objects)
    }
    /// Check if there are any colliding links
    pub fn has_any_colliding(&self, objects: &Compound3<f64>) -> bool {
        for shape in objects.shapes() {
            if self.collision_checker.has_any_colliding(
                &self.collision_check_robot,
                &*shape.1,
                &shape.0,
            )
            {
                return true;
            }
        }
        false
    }
    /// Get the names of colliding links
    pub fn get_colliding_link_names(&self, objects: &Compound3<f64>) -> Vec<String> {
        let mut ret = Vec::new();
        for shape in objects.shapes() {
            let mut colliding_names = self.collision_checker.get_colliding_link_names(
                &self.collision_check_robot,
                &*shape.1,
                &shape.0,
            );
            ret.append(&mut colliding_names);
        }
        ret
    }

    /// Plan the sequence of joint angles of `using_joints`
    ///
    /// # Arguments
    ///
    /// - `start_angles`: initial joint angles of `using_joints`.
    /// - `goal_angles`: goal joint angles of `using_joints`.
    /// - `objects`: The collision between `self.collision_check_robot` and `objects`
    ///   will be checked.
    pub fn plan<K>(
        &mut self,
        using_joints: &mut K,
        start_angles: &[f64],
        goal_angles: &[f64],
        objects: &Compound3<f64>,
    ) -> Result<Vec<Vec<f64>>>
    where
        K: k::JointContainer<f64>,
    {
        let limits = using_joints.get_joint_limits();
        let step_length = self.step_length;
        let max_try = self.max_try;
        let current_angles = using_joints.get_joint_angles();
        if !self.is_feasible(using_joints, start_angles, objects) {
            using_joints.set_joint_angles(&current_angles)?;
            return Err(Error::Collision("Initialis colliding".to_owned()));
        } else if !self.is_feasible(using_joints, goal_angles, objects) {
            using_joints.set_joint_angles(&current_angles)?;
            return Err(Error::Collision("Goal is colliding".to_owned()));
        }
        let mut path = match rrt::dual_rrt_connect(
            start_angles,
            goal_angles,
            |angles: &[f64]| self.is_feasible(using_joints, angles, objects),
            || generate_random_joint_angles_from_limits(&limits),
            step_length,
            max_try,
        ) {
            Ok(p) => p,
            Err(error) => {
                using_joints.set_joint_angles(&current_angles)?;
                return Err(Error::from(error));
            }
        };
        let num_smoothing = self.num_smoothing;
        rrt::smooth_path(
            &mut path,
            |angles: &[f64]| self.is_feasible(using_joints, angles, objects),
            step_length,
            num_smoothing,
        );
        Ok(path)
    }
}

impl<R> k::LinkContainer<f64> for JointPathPlanner<R>
where
    R: k::LinkContainer<f64>,
{
    /// Calculate the transforms of all of the links
    fn calc_link_transforms(&self) -> Vec<na::Isometry3<f64>> {
        self.collision_check_robot.calc_link_transforms()
    }

    /// Get the names of the links
    fn get_link_names(&self) -> Vec<String> {
        self.collision_check_robot.get_link_names()
    }
}

/// Builder pattern to create `JointPathPlanner`
pub struct JointPathPlannerBuilder<R>
where
    R: k::LinkContainer<f64>,
{
    collision_check_robot: R,
    collision_checker: CollisionChecker<f64>,
    step_length: f64,
    max_try: usize,
    num_smoothing: usize,
    collision_check_margin: Option<f64>,
    urdf_robot: Option<urdf_rs::Robot>,
}

impl<R> JointPathPlannerBuilder<R>
where
    R: k::LinkContainer<f64>,
{
    /// Create from components
    ///
    /// There are also some utility functions to create from urdf
    pub fn new(collision_check_robot: R, collision_checker: CollisionChecker<f64>) -> Self {
        JointPathPlannerBuilder {
            collision_check_robot: collision_check_robot,
            collision_checker: collision_checker,
            step_length: 0.1,
            max_try: 5000,
            num_smoothing: 100,
            collision_check_margin: None,
            urdf_robot: None,
        }
    }
    pub fn collision_check_margin(mut self, length: f64) -> Self {
        self.collision_check_margin = Some(length);
        self
    }
    pub fn step_length(mut self, step_length: f64) -> Self {
        self.step_length = step_length;
        self
    }
    pub fn max_try(mut self, max_try: usize) -> Self {
        self.max_try = max_try;
        self
    }
    pub fn num_smoothing(mut self, num_smoothing: usize) -> Self {
        self.num_smoothing = num_smoothing;
        self
    }
    pub fn finalize(mut self) -> JointPathPlanner<R> {
        let mut planner = JointPathPlanner::new(
            self.collision_check_robot,
            self.collision_checker,
            self.step_length,
            self.max_try,
            self.num_smoothing,
        );
        if let Some(margin) = self.collision_check_margin {
            self.collision_checker.prediction = margin;
        }
        planner.urdf_robot = self.urdf_robot;
        planner
    }
}

impl<R> JointPathPlannerBuilder<R>
where
    R: k::urdf::FromUrdf + k::LinkContainer<f64>,
{
    /// Try to create `JointPathPlannerBuilder` instance from URDF file and end link name
    pub fn from_urdf_file<P>(file: P) -> Result<JointPathPlannerBuilder<R>>
    where
        P: AsRef<Path>,
    {
        let robot = urdf_rs::utils::read_urdf_or_xacro(file.as_ref())?;
        get_joint_path_planner_builder_from_urdf(robot)
    }
    /// Try to create `JointPathPlannerBuilder` instance from `urdf_rs::Robot` instance
    pub fn from_urdf_robot<P>(robot: urdf_rs::Robot) -> Result<JointPathPlannerBuilder<R>> {
        get_joint_path_planner_builder_from_urdf(robot)
    }
}

fn get_joint_path_planner_builder_from_urdf<R>(
    urdf_robot: urdf_rs::Robot,
) -> Result<JointPathPlannerBuilder<R>>
where
    R: k::urdf::FromUrdf + k::LinkContainer<f64>,
{
    const DEFAULT_MARGIN: f64 = 0.0;
    let collision_checker = CollisionChecker::from_urdf_robot(&urdf_robot, DEFAULT_MARGIN);
    let collision_check_robot = R::from_urdf_robot(&urdf_robot);
    Ok(JointPathPlannerBuilder {
        collision_check_robot,
        collision_checker,
        step_length: 0.1,
        max_try: 5000,
        num_smoothing: 100,
        collision_check_margin: None,
        urdf_robot: Some(urdf_robot),
    })
}

pub type DefaultJointPathPlanner<T> = JointPathPlanner<k::LinkTree<T>>;
pub type DefaultJointPathPlannerBuilder<T> = JointPathPlannerBuilder<k::LinkTree<T>>;


#[cfg(test)]
mod tests {
    use super::*;
    use urdf_rs;
    use na;
    use ncollide::shape::Cuboid;
    use na::{Isometry3, Vector3};
    use k::JointContainer;
    use k::urdf::FromUrdf;

    #[test]
    fn collision_check() {
        let urdf_robot = urdf_rs::read_file("sample.urdf").unwrap();
        let checker = CollisionChecker::from_urdf_robot(&urdf_robot, 0.05);

        let target = Cuboid::new(Vector3::new(0.5, 1.0, 0.5));
        let target_pose = Isometry3::new(Vector3::new(0.9, 0.0, 0.0), na::zero());

        let mut robot = k::LinkTree::<f32>::from_urdf_robot(&urdf_robot);

        let names = checker.get_colliding_link_names(&robot, &target, &target_pose);
        assert_eq!(
            names,
            vec![
                "l_elbow1",
                "l_wrist1",
                "l_wrist2",
                "l_gripper2",
                "l_gripper1",
            ]
        );
        let angles = vec![-1.3, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        robot.set_joint_angles(&angles).unwrap();
        let names = checker.get_colliding_link_names(&robot, &target, &target_pose);
        assert_eq!(
            names,
            vec!["l_wrist1", "l_wrist2", "l_gripper2", "l_gripper1"]
        );
        let target_pose = Isometry3::new(Vector3::new(0.7, 0.0, 0.0), na::zero());
        let names = checker.get_colliding_link_names(&robot, &target, &target_pose);
        assert_eq!(
            names,
            vec![
                "l_shoulder2",
                "l_shoulder3",
                "l_elbow1",
                "l_wrist1",
                "l_wrist2",
                "l_gripper2",
                "l_gripper1",
            ]
        );
    }
    #[test]
    fn from_urdf() {
        let _planner = DefaultJointPathPlannerBuilder::from_urdf_file("sample.urdf")
            .unwrap()
            .collision_check_margin(0.01)
            .finalize();
    }
}
