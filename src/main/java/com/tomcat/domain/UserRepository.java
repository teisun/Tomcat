package com.tomcat.domain;

import com.tomcat.controller.response.UserDTO;
import org.springframework.data.jpa.repository.JpaRepository;

import java.util.List;
import java.util.Optional;

public interface UserRepository extends JpaRepository<User, Long> {
  Optional<User> findByUsername(String username);

  Optional<User> findById(String userId);

  Optional<User> findByPhoneNumOrEmail(String phoneNum, String email);

  // 通过设备ID查询用户
  List<User> findByDeviceId(String deviceId);

}