package com.tomcat.domain;

import org.springframework.data.jpa.repository.JpaRepository;

import java.util.List;
import java.util.Optional;

public interface UserRepository extends JpaRepository<User, Long> {
  Optional<User> findByUsername(String username);

  Optional<User> findByPhoneNumOrEmail(String phoneNum, String email);

  List<User> findByDeviceIdsContains(String deviceId);
}