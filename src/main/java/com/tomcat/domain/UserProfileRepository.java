package com.tomcat.domain;

import org.springframework.data.jpa.repository.JpaRepository;

public interface UserProfileRepository extends JpaRepository<UserProfile, Long> {

  UserProfile findByUserId(String userId);

}